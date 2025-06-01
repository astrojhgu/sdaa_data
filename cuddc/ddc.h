#ifndef DDC_H_
#define DDC_H_
typedef short int int16_t;

struct fcomplex
{
    float real;
    float imag;
};

struct DDCResources;


#ifdef __cplusplus
extern "C"
{
#endif
    struct DDCResources* init_ddc_resources(int N, int M, int NDEC, int K, const float *fir_coeffs);
    void free_ddc_resources(struct DDCResources *res);
    int ddc(const int16_t *indata, int lo_ch, struct DDCResources *res);
    void fetch_output(struct fcomplex *outdata, struct DDCResources *res);
    int calc_output_size(const struct DDCResources *res);

#ifdef __cplusplus
}
#endif

#endif