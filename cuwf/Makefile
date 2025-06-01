all: test_cuwf libcuwf.a #libcuwf.so

OPT=-O3
CFLAGS = -g $(OPT)
LIBS=-L ./lib -lcudart -lcuda -lcufft

test_cuwf.o: test_cuwf.cpp cuwf.h
	g++ -c $< -o $@ $(CFLAGS)

waterfall.o: waterfall.cu cuwf.h
	nvcc --compiler-options -fPIC -c $< -o $@ $(CFLAGS) --cudart=static --cudadevrt=none

test_cuwf: test_cuwf.o waterfall.o
	nvcc $^ -o $@ $(CFLAGS) --cudart=static --cudadevrt=none $(LIBS)

libcuwf.so: waterfall.o
	g++ --shared -fPIC -o $@ $^ $(LIBS)

libcuwf.a: waterfall.o
	ar crv $@ $^
	ranlib $@

clean:
	rm -f *.o
