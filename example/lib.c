#include <stdio.h>

void putf(double f) { printf("%f\n", f); }

void puti(int i) { printf("%d\n", i); }

void print_array(char *v[], int count) {
  for (int i = 0; i < count; i++) {
    printf("%s\n", v[i]);
  }
}