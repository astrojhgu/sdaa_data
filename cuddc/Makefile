all: ${OUT_DIR}/libcuddc.a ${OUT_DIR}/test_ddc #libcuddc.so

OPT=-O3
CFLAGS = -g $(OPT)
LIBS=-L ./lib -lcudart_static -lcuda

${OUT_DIR}/test_ddc.o: test_ddc.cpp
	g++ -c $< -o $@ $(CFLAGS)

${OUT_DIR}/ddc_kernel.o: ddc_kernel.cu
	nvcc --compiler-options -fPIC -c $< -o $@ $(CFLAGS) --cudart=static --cudadevrt=none

${OUT_DIR}/test_ddc: ${OUT_DIR}/test_ddc.o ${OUT_DIR}/ddc_kernel.o
	nvcc $^ -o $@ $(CFLAGS) --cudart=static --cudadevrt=none $(LIBS)

${OUT_DIR}/libcuddc.so: ${OUT_DIR}/ddc_kernel.o
	g++ --shared -fPIC -o $@ $^ $(LIBS)

${OUT_DIR}/libcuddc.a: ${OUT_DIR}/ddc_kernel.o
	ar crv $@ $^
	ranlib $@

clean:
	rm -f ${OUT_DIR}/*.o
