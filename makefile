make:
	cargo run
	gcc test.o lib.c -lm -o test.exe
	./test.exe