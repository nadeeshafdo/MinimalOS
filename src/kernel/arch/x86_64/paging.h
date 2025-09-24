#ifndef PAGING_H
#define PAGING_H

#include "../../stdint.h"
#include "../../stddef.h"

void setup_paging(void);
void map_user_stack(uint64_t addr, size_t size);

#endif