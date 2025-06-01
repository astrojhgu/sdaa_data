#ifndef WATERFALL_H
#define WATERFALL_H
// waterfall.h
#include <stddef.h>
#include <stdbool.h>
#include <stdint.h>


struct Resource;

#ifdef __cplusplus
extern "C"
{
#endif

    struct Resource* init_resource(int nch, int n_pt_per_payload, int nbatch, int nint);
    void destroy_resource(struct Resource *res);
    bool waterfall(struct Resource *res, const int16_t *time_domain_input, size_t npt, float *output_spectrum);

#ifdef __cplusplus
}
#endif

#endif
