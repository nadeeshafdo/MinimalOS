#include "string.h"

void* memset(void* dest, int val, size_t count) {
    u8* d = (u8*)dest;
    while (count--) {
        *d++ = (u8)val;
    }
    return dest;
}

void* memcpy(void* dest, const void* src, size_t count) {
    u8* d = (u8*)dest;
    const u8* s = (const u8*)src;
    while (count--) {
        *d++ = *s++;
    }
    return dest;
}

void* memmove(void* dest, const void* src, size_t count) {
    u8* d = (u8*)dest;
    const u8* s = (const u8*)src;
    
    if (d < s) {
        while (count--) {
            *d++ = *s++;
        }
    } else {
        d += count;
        s += count;
        while (count--) {
            *--d = *--s;
        }
    }
    return dest;
}

int memcmp(const void* s1, const void* s2, size_t count) {
    const u8* a = (const u8*)s1;
    const u8* b = (const u8*)s2;
    while (count--) {
        if (*a != *b) {
            return *a - *b;
        }
        a++;
        b++;
    }
    return 0;
}

size_t strlen(const char* str) {
    size_t len = 0;
    while (str[len]) {
        len++;
    }
    return len;
}

char* strcpy(char* dest, const char* src) {
    char* d = dest;
    while ((*d++ = *src++));
    return dest;
}

char* strncpy(char* dest, const char* src, size_t count) {
    char* d = dest;
    while (count && (*d++ = *src++)) {
        count--;
    }
    while (count--) {
        *d++ = 0;
    }
    return dest;
}

int strcmp(const char* s1, const char* s2) {
    while (*s1 && (*s1 == *s2)) {
        s1++;
        s2++;
    }
    return *(const u8*)s1 - *(const u8*)s2;
}

int strncmp(const char* s1, const char* s2, size_t count) {
    while (count && *s1 && (*s1 == *s2)) {
        s1++;
        s2++;
        count--;
    }
    if (count == 0) {
        return 0;
    }
    return *(const u8*)s1 - *(const u8*)s2;
}
