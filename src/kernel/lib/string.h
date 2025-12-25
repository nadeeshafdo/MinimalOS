#ifndef STRING_H
#define STRING_H

#include "../include/types.h"

void* memset(void* dest, int val, size_t count);
void* memcpy(void* dest, const void* src, size_t count);
void* memmove(void* dest, const void* src, size_t count);
int memcmp(const void* s1, const void* s2, size_t count);

size_t strlen(const char* str);
char* strcpy(char* dest, const char* src);
char* strncpy(char* dest, const char* src, size_t count);
int strcmp(const char* s1, const char* s2);
int strncmp(const char* s1, const char* s2, size_t count);

#endif // STRING_H
