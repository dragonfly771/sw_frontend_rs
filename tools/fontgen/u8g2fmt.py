"""u8g2 font format: encoder (from a bitmap glyph set) and decoder (mirrors
the u8g2-fonts crate reader) - used to build the LARUS display fonts."""
import re, ast

class BitReader:
    def __init__(s, data): s.d=data; s.p=0; s.bit=8; s.cur=0
    def u(s, bits):
        bs=s.bit; be=bs+bits; v=(s.cur>>bs)&0xff
        if be>=8:
            v2=s.d[s.p]; s.p+=1; be-=8; s.cur=v2; v|=(v2<<(8-bs))&0xff
        s.bit=be
        return v & ((1<<bits)-1)
    def s(s_, bits): return s_.u(bits)-(1<<(bits-1))

def header(d):
    k='cnt mode m0 m1 bw bh bx by bd fw fh fx fy asc desc ascp descp'.split()
    h={}
    for i,n in enumerate(k):
        v=d[i]
        if i>=11: v=v-256 if v>127 else v
        h[n]=v
    h['offA']=(d[17]<<8)|d[18]; h['offa']=(d[19]<<8)|d[20]; h['off100']=(d[21]<<8)|d[22]
    return h

def decode(d):
    """-> header, {code: (w,h,xoff,yoff,adv,rows)} rows = list of strings"""
    h=header(d); out={}
    p=23
    while d[p]!=0:
        code=d[p]; jump=d[p+1]
        r=BitReader(d[p+2:p+jump+8])
        w=r.u(h['bw']); hh=r.u(h['bh']); x=r.s(h['bx']); y=r.s(h['by']); a=r.s(h['bd'])
        px=[]
        if w>0 and hh>0:
            n=w*hh
            while len(px)<n:
                a0=r.u(h['m0']); b1=r.u(h['m1'])
                while True:
                    px+= [0]*a0 + [1]*b1
                    if len(px)>=n: break
                    if r.u(1)==0: break
                if len(px)>=n: break
            px=px[:n]
        rows=[''.join('#' if px[j*w+i] else '.' for i in range(w)) for j in range(hh)]
        out[code]=(w,hh,x,y,a,rows)
        p+=jump
    return h,out

class BitWriter:
    def __init__(s): s.bytes=bytearray(); s.acc=0; s.n=0
    def u(s, v, bits):
        assert 0<=v<(1<<bits), (v,bits)
        s.acc|=v<<s.n; s.n+=bits
        while s.n>=8:
            s.bytes.append(s.acc&0xff); s.acc>>=8; s.n-=8
    def s(s_, v, bits): s_.u(v+(1<<(bits-1)), bits)
    def done(s):
        if s.n>0: s.bytes.append(s.acc&0xff); s.acc=0; s.n=0
        return bytes(s.bytes)

def ubits(v):
    b=1
    while (1<<b)<=v: b+=1
    return b
def sbits(lo,hi):
    b=1
    while not (-(1<<(b-1))<=lo and hi<(1<<(b-1))): b+=1
    return b

def runs(px, m0, m1):
    """run length pairs like bdf_rle.c: (zeros, ones), each limited"""
    max0=(1<<m0)-1; max1=(1<<m1)-1
    out=[]; i=0; n=len(px)
    while i<n:
        a=0
        while i<n and px[i]==0 and a<max0: a+=1; i+=1
        b=0
        if a<max0 or True:
            while i<n and px[i]==1 and b<max1: b+=1; i+=1
        out.append((a,b))
    return out

def glyph_bits(g, f, m0, m1):
    w,h,x,y,a,px=g
    bw=BitWriter()
    bw.u(w,f['bw']); bw.u(h,f['bh']); bw.s(x,f['bx']); bw.s(y,f['by']); bw.s(a,f['bd'])
    if w>0 and h>0:
        rl=runs(px,m0,m1)
        prev=None
        for k,pr in enumerate(rl):
            if k==0:
                bw.u(pr[0],m0); bw.u(pr[1],m1)
            elif pr==prev:
                bw.u(1,1)
            else:
                bw.u(0,1); bw.u(pr[0],m0); bw.u(pr[1],m1)
            prev=pr
        bw.u(0,1)
    return bw.done()

def encode(glyphs):
    """glyphs: {code(<256): (w,h,xoff,yoff,adv,pixels list 0/1 row-major)}"""
    codes=sorted(glyphs)
    G=[glyphs[c] for c in codes]
    f={}
    f['bw']=ubits(max(g[0] for g in G)); f['bh']=ubits(max(g[1] for g in G))
    f['bx']=sbits(min(g[2] for g in G), max(g[2] for g in G))
    f['by']=sbits(min(g[3] for g in G), max(g[3] for g in G))
    f['bd']=sbits(min(g[4] for g in G), max(g[4] for g in G))
    best=None
    for m0 in range(2,9):
        for m1 in range(2,9):
            tot=sum(len(glyph_bits(g,f,m0,m1))+2 for g in G)
            if best is None or tot<best[0]: best=(tot,m0,m1)
    _,m0,m1=best
    body=bytearray(); offA=offa=0
    for c,g in zip(codes,G):
        if c==ord('A'): offA=len(body)
        if c==ord('a'): offa=len(body)
        gb=glyph_bits(g,f,m0,m1)
        assert len(gb)+2<256
        body+=bytes([c,len(gb)+2])+gb
    term=len(body)
    body+=b'\x00\x00'
    off100=len(body)
    body+=b'\x00\x04\xff\xff\x00\x00'
    vis=[g for g in G if g[0]>0 and g[1]>0]
    x0=min(g[2] for g in vis); y0=min(g[3] for g in vis)
    x1=max(g[2]+g[0] for g in vis); y1=max(g[3]+g[1] for g in vis)
    def s8(v): return v&0xff
    def asc(c): g=glyphs.get(ord(c)); return g[1]+g[3] if g else 0
    def desc(c): g=glyphs.get(ord(c)); return g[3] if g else 0
    hdr=bytes([len(codes),0,m0,m1,f['bw'],f['bh'],f['bx'],f['by'],f['bd'],
               x1-x0, y1-y0, s8(x0), s8(y0),
               s8(asc('A') if 'A' else 0), s8(desc('g')), s8(asc('(')), s8(desc('('))])
    hdr+=bytes([offA>>8,offA&0xff,offa>>8,offa&0xff,off100>>8,off100&0xff])
    return hdr+bytes(body)

def crate_font(name, path):
    src=open(path,encoding='latin-1').read()
    m=re.search(r'pub struct %s;\nimpl Font for %s \{\n    const DATA: &\'static \[u8\] = (b".*?");\n\}'%(name,name),src,re.S)
    return ast.literal_eval(m.group(1))
