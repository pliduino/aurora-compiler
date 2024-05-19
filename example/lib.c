#include <stdio.h>

void putf(double f) { printf("%f", f); }

void print_array(char *v[], int count) {
  for (int i = 0; i < count; i++) {
    printf("%s\n", v[i]);
  }
}