import serial,time
pays=[b'\x8fx%09d%08d%04d%03d'%(100000000,1000000,10,20), b'\x8fv', b'AT\r\n', b'?', b'v', b'\x8f?']
for baud in (4800,9600,19200,38400,57600,115200,230400,460800,921600,1000000,2000000):
    try: s=serial.Serial('/dev/ttyUSB0',baud,timeout=0.2)
    except Exception as e: print(baud,e); continue
    s.dtr=True; s.rts=True; got=b''
    for p in pays:
        s.reset_input_buffer(); s.write(p); s.flush(); time.sleep(0.7); got+=s.read(512)
    print(baud,len(got),got[:32].hex()); s.close()
