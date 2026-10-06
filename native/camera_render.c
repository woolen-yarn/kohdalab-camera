/* Exact 8-bit bilinear Bayer and correction-LUT renderer. No sensor mutation. */
#include <stdint.h>
#include <stddef.h>
#ifdef _WIN32
#define API __declspec(dllexport)
#else
#define API __attribute__((visibility("default")))
#endif
API int kohda_render_api_version(void) { return 1; }
API int kohda_render(const uint8_t *raw, int width, int height, int pattern,
                     int gray, const uint8_t *lut, int mirror, int flip,
                     int rotation, uint8_t *output) {
    static const char patterns[4][5] = {"GRBG", "GBRG", "RGGB", "BGGR"};
    if (!raw || !lut || !output || width < 2 || height < 2 || pattern < 0 || pattern > 3 ||
        (rotation != 0 && rotation != 90 && rotation != 180 && rotation != 270)) return -1;
    const char *sites = patterns[pattern];
    int rr=0, rc=0, br=0, bc=0;
    for (int i=0; i<4; ++i) {
        if (sites[i]=='R') { rr=i/2; rc=i%2; }
        if (sites[i]=='B') { br=i/2; bc=i%2; }
    }
    int output_width=(rotation==90 || rotation==270) ? height : width;
    for (int y=0; y<height; ++y) {
        int ym=y ? y-1 : 1, yp=y+1<height ? y+1 : height-2;
        for (int x=0; x<width; ++x) {
            int xm=x ? x-1 : 1, xp=x+1<width ? x+1 : width-2;
            unsigned own=raw[(size_t)y*width+x];
            unsigned r=own, g=own, b=own;
            if (!gray) {
                unsigned horizontal=raw[(size_t)y*width+xm]+raw[(size_t)y*width+xp];
                unsigned vertical=raw[(size_t)ym*width+x]+raw[(size_t)yp*width+x];
                unsigned diagonal=raw[(size_t)ym*width+xm]+raw[(size_t)ym*width+xp]+
                                  raw[(size_t)yp*width+xm]+raw[(size_t)yp*width+xp];
                int row=y&1, col=x&1;
                r=(row==rr) ? ((col==rc) ? own : horizontal>>1) : ((col==rc) ? vertical>>1 : diagonal>>2);
                b=(row==br) ? ((col==bc) ? own : horizontal>>1) : ((col==bc) ? vertical>>1 : diagonal>>2);
                g=sites[row*2+col]=='G' ? own : (horizontal+vertical)>>2;
            }
            int ox=mirror ? width-1-x : x, oy=flip ? height-1-y : y;
            int dx=ox, dy=oy;
            if (rotation==90) { dx=height-1-oy; dy=ox; }
            else if (rotation==180) { dx=width-1-ox; dy=height-1-oy; }
            else if (rotation==270) { dx=oy; dy=width-1-ox; }
            size_t offset=((size_t)dy*output_width+dx)*3;
            output[offset]=lut[r*3]; output[offset+1]=lut[g*3+1]; output[offset+2]=lut[b*3+2];
        }
    }
    return 0;
}
