all: ${OUT_DIR}/libcuwf.a #${OUT_DIR}/test_cuwf  libcuwf.so

OPT=-O3
CFLAGS = -g $(OPT)
LIBS=-L ./lib -lcudart_static -lcuda -lcufft_static_nocallback -lculibos

${OUT_DIR}/test_cuwf.o: test_cuwf.cpp cuwf.h
	g++ -c $< -o $@ $(CFLAGS)

${OUT_DIR}/waterfall.o: waterfall.cu cuwf.h
	nvcc --compiler-options -fPIC -c $< -o $@ $(CFLAGS) --cudart=static --cudadevrt=none

test_cuwf: ${OUT_DIR}/test_cuwf.o ${OUT_DIR}/waterfall.o
	nvcc $^ -o $@ $(CFLAGS) --cudart=static --cudadevrt=none $(LIBS)

${OUT_DIR}/libcuwf.so: ${OUT_DIR}/waterfall.o
	g++ --shared -fPIC -o $@ $^ $(LIBS)

${OUT_DIR}/libcuwf.a: ${OUT_DIR}/waterfall.o
	ar crv $@ $^
	ranlib $@

clean:
	rm -f ${OUT_DIR}/*.o
