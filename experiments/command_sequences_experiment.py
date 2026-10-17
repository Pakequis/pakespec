import sys,time,serial
s=serial.Serial(sorted(__import__('glob').glob('/dev/ttyUSB*'))[0],57600,timeout=0.1); s.dtr=s.rts=True
X=b'\x8fx035000000001000000011'  # 11 points
V=b'\x8fv'; F0=b'\x8ff000000000'
def cmd(p,wait=0.6):
    s.reset_input_buffer(); s.write(p); s.flush(); time.sleep(wait); r=s.read(4096); return r
def show(name,seq):
    out=[]
    for p in seq:
        r=cmd(p); out.append(f'{"X" if p==X else "V" if p==V else "F0"}:{len(r)}')
    print(name,' '.join(out),flush=True)
show('X',[X]); show('X X',[X,X]); show('X X X',[X,X,X]); show('V X',[V,X]); show('V',[V,V,V]); show('X',[X]); show('F0 X',[F0,X]); show('F0 V X',[F0,V,X]); show('X V X',[X,V,X]); show('X F0 V X',[X,F0,V,X]); show('X',[X]); show('X',[X])
