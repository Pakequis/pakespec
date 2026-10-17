import time,serial,glob
s=serial.Serial(sorted(glob.glob('/dev/ttyUSB*'))[0],57600,timeout=0.1); s.dtr=s.rts=True
X=b'\x8fx035000000001000000011'
def drain(): 
    time.sleep(0.3); s.reset_input_buffer()
def t(name,payload,wait=0.6):
    drain(); s.write(payload); s.flush(); time.sleep(wait); r=s.read(4096); print(f'{name:<14} -> {len(r)} bytes',r[:8].hex(),flush=True)
t('X alone',X)
t('X + 0x8F',X+b'\x8f')
t('X + \\n',X+b'\n')
t('X + \\r',X+b'\r')
t('X + space',X+b' ')
t('X + 0x00',X+b'\x00')
t('X + 0x8F v',X+b'\x8fv')
t('X alone',X)
t('X + 0x8F',X+b'\x8f')
