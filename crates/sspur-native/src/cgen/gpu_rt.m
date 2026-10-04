#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
typedef struct { int32_t kind, pad; const void* ptr; int64_t bytes; void* dev; } SsGpuArg;
#ifdef __OBJC__
#import <Metal/Metal.h>
static id<MTLDevice> g_dev;
static id<MTLCommandQueue> g_queue;
static NSMutableDictionary* g_pipes;
static int g_state = -1;
static int trace_on(void) { const char* t = getenv("SSPUR_GPU_TRACE"); return t && *t && *t != '0'; }
int ss_gpu_ready(void) {
    if (g_state >= 0) return g_state;
    g_state = 0;
    const char* e = getenv("SSPUR_GPU");
    if (e && *e == '0') return 0;
    @autoreleasepool {
        g_dev = MTLCreateSystemDefaultDevice();
        if (!g_dev) return 0;
        g_queue = [g_dev newCommandQueue];
        g_pipes = [NSMutableDictionary new];
        if (trace_on()) fprintf(stderr, "gpu: %s\n", [[g_dev name] UTF8String]);
    }
    g_state = g_queue != nil;
    return g_state;
}
static NSMutableDictionary* g_libs;
static id<MTLComputePipelineState> pipe_for(const char* src, const char* entry) {
    NSString* key = [NSString stringWithFormat:@"%s\n%s", entry, src];
    id p = g_pipes[key];
    if (p) return p == [NSNull null] ? nil : p;
    if (!g_libs) g_libs = [NSMutableDictionary new];
    NSString* skey = [NSString stringWithUTF8String:src];
    id lib = g_libs[skey];
    NSError* err = nil;
    if (!lib) {
        MTLCompileOptions* o = [MTLCompileOptions new];
        if (@available(macOS 15.0, *)) {
            o.mathMode = MTLMathModeSafe;
            o.mathFloatingPointFunctions = MTLMathFloatingPointFunctionsPrecise;
        }
        lib = [g_dev newLibraryWithSource:skey options:o error:&err];
        if (!lib && trace_on()) fprintf(stderr, "gpu: cannot compile kernels: %s\n", err ? [[err description] UTF8String] : "unknown error");
        g_libs[skey] = lib ? lib : [NSNull null];
    }
    id<MTLComputePipelineState> ps = nil;
    if (lib != [NSNull null]) {
        id<MTLFunction> fn = [(id<MTLLibrary>)lib newFunctionWithName:[NSString stringWithUTF8String:entry]];
        if (fn) ps = [g_dev newComputePipelineStateWithFunction:fn error:&err];
    }
    g_pipes[key] = ps ? (id)ps : (id)[NSNull null];
    return ps;
}
int ss_gpu_run(const char* src, const char* entry, int32_t nargs, const SsGpuArg* args, int64_t n, int64_t group) {
    if (!ss_gpu_ready() || n <= 0 || n > 0x7fffffff) return 2;
    @autoreleasepool {
        id<MTLComputePipelineState> ps = pipe_for(src, entry);
        if (!ps) return 2;
        NSMutableArray* bufs = [NSMutableArray arrayWithCapacity:nargs];
        id<MTLCommandBuffer> cb = [g_queue commandBuffer];
        id<MTLComputeCommandEncoder> en = [cb computeCommandEncoder];
        [en setComputePipelineState:ps];
        for (int32_t i = 0; i < nargs; i++) {
            const SsGpuArg* a = &args[i];
            id<MTLBuffer> b = nil;
            if (a->kind == 0) {
                [en setBytes:a->ptr length:(NSUInteger)a->bytes atIndex:(NSUInteger)i];
                [bufs addObject:[NSNull null]];
                continue;
            }
            if (a->kind == 3) b = (__bridge id<MTLBuffer>)a->dev;
            else if (a->bytes > 0) b = [g_dev newBufferWithBytes:a->ptr length:(NSUInteger)a->bytes options:MTLResourceStorageModeShared];
            else b = [g_dev newBufferWithLength:16 options:MTLResourceStorageModeShared];
            if (!b) { [en endEncoding]; return 2; }
            [en setBuffer:b offset:0 atIndex:(NSUInteger)i];
            [bufs addObject:b];
        }
        id<MTLBuffer> flag = [g_dev newBufferWithLength:4 options:MTLResourceStorageModeShared];
        memset([flag contents], 0, 4);
        [en setBuffer:flag offset:0 atIndex:(NSUInteger)nargs];
        NSUInteger g = ps.maxTotalThreadsPerThreadgroup;
        if ((int64_t)g > group) g = (NSUInteger)group;
        if ((int64_t)g > n) g = (NSUInteger)n;
        [en dispatchThreads:MTLSizeMake((NSUInteger)n, 1, 1) threadsPerThreadgroup:MTLSizeMake(g, 1, 1)];
        [en endEncoding];
        [cb commit];
        [cb waitUntilCompleted];
        if (cb.status != MTLCommandBufferStatusCompleted) {
            if (trace_on()) fprintf(stderr, "gpu: %s failed: %s\n", entry, cb.error ? [[cb.error description] UTF8String] : "unknown error");
            return 2;
        }
        if (*(uint32_t*)[flag contents]) return 1;
        for (int32_t i = 0; i < nargs; i++) {
            if (args[i].kind == 2) memcpy((void*)args[i].ptr, [(id<MTLBuffer>)bufs[i] contents], (size_t)args[i].bytes);
        }
    }
    return 0;
}
void* ss_gpu_alloc(int64_t bytes, void** host) {
    if (!ss_gpu_ready()) return 0;
    id<MTLBuffer> b = [g_dev newBufferWithLength:(NSUInteger)(bytes > 16 ? bytes : 16) options:MTLResourceStorageModeShared];
    if (!b) return 0;
    *host = [b contents];
    return (__bridge_retained void*)b;
}
void ss_gpu_free(void* dev) {
    if (dev) CFRelease(dev);
}
#else
int ss_gpu_ready(void) { return 0; }
int ss_gpu_run(const char* src, const char* entry, int32_t nargs, const SsGpuArg* args, int64_t n, int64_t group) { (void)src; (void)entry; (void)nargs; (void)args; (void)n; (void)group; return 2; }
void* ss_gpu_alloc(int64_t bytes, void** host) { (void)bytes; (void)host; return 0; }
void ss_gpu_free(void* dev) { (void)dev; }
#endif
