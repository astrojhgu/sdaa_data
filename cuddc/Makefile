all: test_ddc libcuddc.a #libcuddc.so

OPT=-O3
CFLAGS = -g $(OPT)
LIBS=-L ./lib -lcudart -lcuda

test_ddc.o: test_ddc.cpp
	g++ -c $< -o $@ $(CFLAGS)

ddc_kernel.o: ddc_kernel.cu
	nvcc --compiler-options -fPIC -c $< -o $@ $(CFLAGS) --cudart=static --cudadevrt=none

test_ddc: test_ddc.o ddc_kernel.o
	nvcc $^ -o $@ $(CFLAGS) --cudart=static --cudadevrt=none

libcuddc.so: ddc_kernel.o
	g++ --shared -fPIC -o $@ $^ $(LIBS)

libcuddc.a: ddc_kernel.o
	ar crv $@ $^
	ranlib $@

clean:
	rm -f *.o
