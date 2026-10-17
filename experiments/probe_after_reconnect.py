import serial,time,glob,sys
log=open(sys.argv[1],'w',buffering=1)
def P(*a): print(time.strftime('%H:%M:%S'),*a,file=log)
P('waiting for ttyUSB*')
while not glob.glob('/dev/ttyUSB*'): time.sleep(0.2)
port=sorted(glob.glob('/dev/ttyUSB*'))[0]; P('appeared',port)
time.sleep(1.0)
s=serial.Serial(port,57600,timeout=0.3); s.dtr=True; s.rts=True
# passive listen right after enumeration (boot banner?)
P('boot bytes',s.read(256).hex())
def go(label,pay,wait=2.5,n=1001):
    s.reset_input_buffer(); s.write(pay); s.flush()
    t0=time.time(); buf=b''
    while time.time()-t0<wait: buf+=s.read(512)
    P(label,len(buf),buf[:40].hex()); return buf
go('v',b'\x8fv')
go('f',b'\x8ff000000000',1)
go('v2',b'\x8fv')
go('x sweep 35M..1G 1001pts',b'\x8fx035000000009650001001000',6)
go('x small 35M..45M 11pts',b'\x8fx035000000001000000011',3)
go('f 50MHz',b'\x8ff050000000',1)
P('done')
