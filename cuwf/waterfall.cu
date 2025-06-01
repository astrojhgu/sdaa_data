#include "cuwf.h"
#include <cuda_runtime.h>
#include <cufft.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// CUDA 错误检查宏
#define CHECK_CUDA(call)                                                                 \
    do {                                                                                 \
        cudaError_t err = (call);                                                        \
        if (err != cudaSuccess) {                                                        \
            fprintf(stderr, "CUDA error at %s:%d: %s\n", __FILE__, __LINE__,             \
                    cudaGetErrorString(err));                                            \
            exit(EXIT_FAILURE);                                                          \
        }                                                                                \
    } while (0)

#define CHECK_CUFFT(call)                                                                \
    do {                                                                                 \
        cufftResult err = (call);                                                        \
        if (err != CUFFT_SUCCESS) {                                                      \
            fprintf(stderr, "CUFFT error at %s:%d: %d\n", __FILE__, __LINE__, err);      \
            exit(EXIT_FAILURE);                                                          \
        }                                                                                \
    } while (0)

struct Resource
{
    int nch;             // FFT点数为 2*nch
    int nbatch;          // 一次处理的batch数
    int nint;
    size_t total_length; // host缓冲区长度 = 2*nch*nbatch
    size_t filled;       // 已填充的数据点数（单位：int16_t个数）

    int16_t *host_buffer;  // host缓存
    int16_t *tmp_overflow; // 溢出缓存
    size_t tmp_len;        // 溢出缓存当前长度

    int16_t *d_raw_input;   // GPU上缓存的原始int16数据
    cufftComplex *d_input;  // GPU输入 (complex<float>)
    cufftComplex *d_output; // GPU输出
    float *d_spectrum;      // GPU上存储最终谱
    float *h_spectrum;      // host端谱输出缓冲区

    cufftHandle fft_plan;
};

// CUDA kernel: int16_t → cufftComplex (real part)，imag = 0
__global__ void convert_int16_to_complex(const int16_t* input, cufftComplex* output, int n) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx < n) {
        output[idx].x = static_cast<float>(input[idx]);
        output[idx].y = 0.0f;
    }
}

// CUDA kernel: 对每个频点，在每组 nint 条谱上做归约，输出 nch * (nbatch/nint) 个值
__global__ void compute_power_spectrum_grouped(
    const cufftComplex* freq_data,
    float* spectrum,
    int nch,
    int nbatch,
    int nint)
{
    int freq_bin = blockIdx.x * blockDim.x + threadIdx.x;
    int group = blockIdx.y;  // 第 group 组
    if (freq_bin >= nch) return;

    float sum = 0.0f;
    int base_idx = group * nint;

    for (int j = 0; j < nint; ++j) {
        int idx = (base_idx + j) * (2 * nch) + freq_bin;
        cufftComplex val = freq_data[idx];
        sum += val.x * val.x + val.y * val.y;
    }

    int out_idx = group * nch + freq_bin;
    spectrum[out_idx] = sum;
}


Resource* init_resource(int nch, int n_pt_per_payload, int nbatch, int nint) {
    Resource* res = (Resource*)malloc(sizeof(Resource));
    res->nch = nch;
    res->nint= nint;
    res->nbatch = nbatch;
    res->total_length = 2 * nch * nbatch;
    res->filled = 0;
    res->tmp_len = 0;

    res->host_buffer = (int16_t*)malloc(sizeof(int16_t) * res->total_length);
    res->tmp_overflow = (int16_t*)malloc(sizeof(int16_t) * n_pt_per_payload);
    res->h_spectrum = (float*)malloc(sizeof(float) * nch);

    CHECK_CUDA(cudaMalloc(&res->d_raw_input, sizeof(int16_t) * res->total_length));
    CHECK_CUDA(cudaMalloc(&res->d_input, sizeof(cufftComplex) * res->total_length));
    CHECK_CUDA(cudaMalloc(&res->d_output, sizeof(cufftComplex) * res->total_length));
    CHECK_CUDA(cudaMalloc(&res->d_spectrum, sizeof(float) * nch*nbatch/nint));

    CHECK_CUFFT(cufftPlan1d(&res->fft_plan, 2 * nch, CUFFT_C2C, nbatch));

    return res;
}

void destroy_resource(Resource* res) {
    free(res->host_buffer);
    free(res->tmp_overflow);
    free(res->h_spectrum);
    CHECK_CUDA(cudaFree(res->d_raw_input));
    CHECK_CUDA(cudaFree(res->d_input));
    CHECK_CUDA(cudaFree(res->d_output));
    CHECK_CUDA(cudaFree(res->d_spectrum));
    CHECK_CUFFT(cufftDestroy(res->fft_plan));
    free(res);
}

bool waterfall(Resource* res, const int16_t* time_domain_input, size_t npt, float* output_spectrum) {
    if (res->filled + npt <= res->total_length) {
        memcpy(res->host_buffer + res->filled, time_domain_input, sizeof(int16_t) * npt);
        res->filled += npt;
        return false;
    }

    size_t first_part = res->total_length - res->filled;
    memcpy(res->host_buffer + res->filled, time_domain_input, sizeof(int16_t) * first_part);
    size_t remaining = npt - first_part;
    memcpy(res->tmp_overflow, time_domain_input + first_part, sizeof(int16_t) * remaining);
    res->tmp_len = remaining;
    res->filled = 0;

    int total_pts = res->total_length;
    int threads = 256;
    int blocks = (total_pts + threads - 1) / threads;

    CHECK_CUDA(cudaMemcpy(res->d_raw_input, res->host_buffer,
                          sizeof(int16_t) * total_pts, cudaMemcpyHostToDevice));

    convert_int16_to_complex<<<blocks, threads>>>(res->d_raw_input, res->d_input, total_pts);
    CHECK_CUDA(cudaGetLastError());

    CHECK_CUFFT(cufftExecC2C(res->fft_plan, res->d_input, res->d_output, CUFFT_FORWARD));

    
    dim3 grid((res->nch + threads - 1) / threads, res->nbatch / res->nint);
    compute_power_spectrum_grouped<<<grid, threads>>>(res->d_output, res->d_spectrum, res->nch, res->nbatch, res->nint);
    CHECK_CUDA(cudaDeviceSynchronize());

    
    // 主机端输出缓冲区大小也需要更新为 nch * (nbatch / nint)
    cudaMemcpyAsync(output_spectrum, res->d_spectrum,
                    sizeof(float) * res->nch * (res->nbatch / res->nint),
                    cudaMemcpyDeviceToHost);
    // 溢出数据回填到缓冲区
    memcpy(res->host_buffer, res->tmp_overflow, sizeof(int16_t) * res->tmp_len);
    res->filled = res->tmp_len;
    res->tmp_len = 0;

    return true;
}
