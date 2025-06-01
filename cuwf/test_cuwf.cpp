// test_waterfall.cpp
#include "cuwf.h"
#include <vector>
#include <iostream>
#include <fstream>
#include <cmath>
#include <cstdlib>


// 定义常量
#define N_PT_PER_PAYLOAD 4096 // 示例大小，可调整

int main()
{
    const int nch = 16384;
    const int nbatch = 1024;
    const int nint=128;
    const int total_samples = 2 * nch * nbatch;

    Resource *res = init_resource(nch, N_PT_PER_PAYLOAD, nbatch, nint);

    //float *output_spectrum = new float[nch];
    std::vector<float> output_spectrum(nch*nbatch/nint);
    int16_t *payload = new int16_t[N_PT_PER_PAYLOAD];
    for(int i=0;i<N_PT_PER_PAYLOAD;++i){
        payload[i]=(i%16)>8?-1:1;
    }

    std::ofstream ofs("a.dat");
    for (int i = 0; i < 4800000000/N_PT_PER_PAYLOAD; ++i)
    {
        bool result = waterfall(res, payload, N_PT_PER_PAYLOAD, output_spectrum.data());
        if(result){
            ofs.write((char*)output_spectrum.data(), sizeof(float)*output_spectrum.size());
        }
    }
}
