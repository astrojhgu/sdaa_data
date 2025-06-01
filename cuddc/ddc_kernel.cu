#include <complex>
#include <cassert>
#include <cuda_runtime.h>
#include <cuComplex.h>
#include <cstdio> // 使用 C++ 风格的头文件

using namespace std;
static constexpr float PI=3.14159265358979323846;

// DDC 处理所需的 GPU 资源
struct DDCResources
{
    int N;  // 每次追加的数据长度
    int M; // 累积多少块数据后计算
    int NDEC;// 下抽样倍率
    int K;   // 滤波器抽头数/下抽样倍率
    int16_t *d_indata; // 设备中的输入数据缓冲区
    cuFloatComplex *d_outdata; // 设备中的输出数据缓冲区
    cuFloatComplex *mixed_data; // 存放混频结果的缓冲区
    float *d_fir_coeffs; // 滤波器系数，位于gpu显存中
    int16_t *h_indata; // 输入数据缓冲区，位于RAM中
    int h_index; // 输入缓冲区尾端索引号，每次追加数据都放入此位置
};

// 复数乘法
__device__ static cuFloatComplex complex_mult(float a, float b, float c, float d)
{
    return make_cuFloatComplex(a * c - b * d, a * d + b * c);
}

__global__ void mix(int16_t *indata, cuFloatComplex *mixed_data, int offset, int N, int M, int lo_ch)
{
    int total_size=N*M;
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < total_size)
    {
        float phase=-(float)(i%N)*(float)lo_ch/(float)N*2.0*PI;
        float lo_cos=cos(phase);
        float lo_sin=sin(phase);
        mixed_data[offset + i] = complex_mult(float(indata[i]), 0.0f, lo_cos, lo_sin);
    }
}

// 设备核函数：FIR 滤波并下抽样
__global__ void fir_filter(cuFloatComplex *mixed_data, cuFloatComplex *outdata, const float *fir_coeffs, int NDEC, int K, int total_size)
{
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    int output_index = i;
    int input_index = i * NDEC;

    if (output_index < total_size / NDEC)
    {
        cuFloatComplex sum = make_cuFloatComplex(0.0f, 0.0f);
        for (int j = 0; j < K * NDEC; j++)
        {
            sum = cuCaddf(sum, cuCmulf(make_cuFloatComplex(fir_coeffs[j], 0.0f), mixed_data[input_index + j]));
        }
        outdata[output_index] = sum;
    }
}

// 初始化 DDC 资源
extern "C" struct DDCResources* init_ddc_resources(int N, int M, int NDEC, int K, const float *fir_coeffs)
{
    DDCResources *res = (DDCResources *)malloc(sizeof(DDCResources));
    res->NDEC = NDEC;
    res->K = K;
    res->N=N;
    res->M=M;
    int buffer_size = M * N + NDEC * (K - 1);
    int fir_size = NDEC * K;

    cudaError_t err = cudaMalloc((void **)&res->d_indata, M * N * sizeof(int16_t));
    assert(err == cudaSuccess);
    err = cudaMalloc((void **)&res->d_outdata, (M * N / NDEC) * sizeof(cuFloatComplex));
    assert(err == cudaSuccess);
    err = cudaMalloc((void **)&res->mixed_data, buffer_size * sizeof(cuFloatComplex));
    assert(err == cudaSuccess);
    err = cudaMalloc((void **)&res->d_fir_coeffs, fir_size * sizeof(float));
    assert(err == cudaSuccess);

    res->h_indata = (int16_t *)malloc(M * N * sizeof(int16_t));
    assert(res->h_indata);
    res->h_index = 0;

    err = cudaMemcpy(res->d_fir_coeffs, fir_coeffs, fir_size * sizeof(float), cudaMemcpyHostToDevice);
    assert(err == cudaSuccess);
    return res;
}

// 释放资源
extern "C" void free_ddc_resources(DDCResources *res)
{
    cudaFree(res->d_indata);
    cudaFree(res->d_outdata);
    cudaFree(res->mixed_data);
    cudaFree(res->d_fir_coeffs);
    free(res->h_indata);
    free(res);
}

// DDC 处理
extern "C" int ddc(const int16_t *indata, int lo_ch, DDCResources *res)
{
    memcpy(res->h_indata + res->h_index, indata, res->N * sizeof(int16_t));
    res->h_index += res->N;

    if (res->h_index == res->M * res->N)
    {
        int total_size = res->M * res->N;
        //int buffer_size = total_size + res->NDEC * (res->K - 1);
        int offset = res->NDEC * (res->K - 1);

        cudaMemcpy(res->d_indata, res->h_indata, total_size * sizeof(int16_t), cudaMemcpyHostToDevice);
        mix<<<(total_size + 255) / 256, 256>>>(res->d_indata, res->mixed_data, offset, res->N, res->M, lo_ch);
        cudaError_t err = cudaGetLastError();
        if (err != cudaSuccess)
            return -1;
        cudaDeviceSynchronize();
        err = cudaGetLastError();
        if (err != cudaSuccess)
            return -1;

        fir_filter<<<(total_size / res->NDEC + 255) / 256, 256>>>(res->mixed_data, res->d_outdata, res->d_fir_coeffs, res->NDEC, res->K, total_size);
        err = cudaGetLastError();
        if (err != cudaSuccess)
            return -1;
        cudaDeviceSynchronize();
        err = cudaGetLastError();
        if (err != cudaSuccess)
            return -1;

        
        res->h_index = 0;
        return 1;
    }
    return 0;
}

extern "C" void fetch_output(std::complex<float> *outdata, DDCResources* res){
    int total_size = res->M * res->N;
    cudaMemcpy(outdata, res->d_outdata, (total_size / res->NDEC) * sizeof(cuFloatComplex), cudaMemcpyDeviceToHost);
}


extern "C" int calc_output_size(const DDCResources* res){
    return (res->M)*(res->N)/(res->NDEC);
}
