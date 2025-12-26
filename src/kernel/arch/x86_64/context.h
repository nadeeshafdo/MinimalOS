#ifndef CONTEXT_H
#define CONTEXT_H

#include "../../process/process.h"

/**
 * Perform context switch between processes
 * @param old_ctx Pointer to pointer where old context should be saved
 * @param new_ctx Pointer to new context to load
 */
void context_switch(cpu_context_t** old_ctx, cpu_context_t* new_ctx);

#endif // CONTEXT_H
