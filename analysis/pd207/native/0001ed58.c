/* WARNING: Globals starting with '_' overlap smaller symbols at the same address */

void FUN_0001ed58(int param_1)

{
  int iVar1;
  int iVar2;
  undefined4 uVar3;
  byte bVar4;
  uint uVar5;
  char cVar6;
  byte *pbVar7;
  int iStack_4;

  pbVar7 = (byte *)(param_1 + 0x180);
  *(undefined4 *)(param_1 + 0x14) = 0x20013ee4;
LAB_0001ed7c:
  do {
    do {
      iVar1 = FUN_0004c7a0(0x20013ee4,0xffffffff,&iStack_4,0xffffffff,5);
    } while (iVar1 != 0);
    do {
      iVar1 = _DAT_2001a5d0;
      if (iStack_4 != 0x10) {
        if (iStack_4 < 0x40) {
          if (iStack_4 == 1) {
            FUN_0000934e();
            iVar2 = thunk_EXT_FUN_1fff8272(param_1,iVar1,*(undefined2 *)(iVar1 + 0x1658));
            if (iVar2 == 2) {
              *(byte *)(param_1 + 0x1a) = *(byte *)(param_1 + 0x1a) | 1;
              FUN_0000eef0(param_1,0,4,0);
              FUN_0004c888(*(undefined4 *)(param_1 + 0x10),8);
            }
            else if (iVar2 == 1) {
              *(byte *)(param_1 + 0x1a) = *(byte *)(param_1 + 0x1a) | 0x80;
              FUN_0000eef0(param_1,0,5,0);
            }
            else if (iVar2 == 0) {
              FUN_0000ecfc(param_1,pbVar7,0,*(undefined1 *)(param_1 + 0x124));
              bVar4 = *(byte *)(param_1 + 0x1b);
              *(byte *)(param_1 + 0x1b) = bVar4 | 0x10;
              if ((int)((uint)*(byte *)(param_1 + 0x1a) << 0x1d) < 0) {
                *(byte *)(param_1 + 0x1b) = bVar4 | 0x90;
              }
              bVar4 = *(byte *)(param_1 + 0x181);
              if (((bVar4 & 0x70) == 0) && ((*pbVar7 & 0x1f) == 1)) {
                if (*(char *)(param_1 + 0x46) == '\x05') {
                  bVar4 = *(byte *)(param_1 + 0x1a) | 4;
                  goto LAB_0001ee90;
                }
              }
              else if ((*(byte *)(param_1 + 0x124) - 1 < 2) &&
                      ((*(byte *)(param_1 + 0x18) & 0x60) != 0x60)) {
                if ((((bVar4 & 0x70) == 0x50) && ((*pbVar7 & 0x1f) == 0xf)) &&
                   ((*(uint *)(param_1 + 0x182) & 0xffff0041) == 0xff000041)) {
                  *(undefined4 *)(param_1 + 0x3ec) = *(undefined4 *)(param_1 + 0x186);
                  *(undefined4 *)(param_1 + 0x3f0) = *(undefined4 *)(param_1 + 0x18a);
                  *(undefined4 *)(param_1 + 0x3f4) = *(undefined4 *)(param_1 + 0x18e);
                  *(undefined4 *)(param_1 + 0x3f8) = *(undefined4 *)(param_1 + 0x192);
                  *(byte *)(param_1 + 1000) = *(byte *)(param_1 + 1000) | 1;
                }
              }
              else if (*(char *)(param_1 + 0x22) == '\x01') {
                if ((int)((uint)*(byte *)(param_1 + 0x18) << 0x1b) < 0) {
                  if (*(byte *)(param_1 + 0x124) == 0) {
                    uVar5 = (uint)*(byte *)(param_1 + 0x26);
                    cVar6 = ((byte)((uVar5 & 0x10) << 1) | *(char *)(param_1 + 0x44) << 6) + 1;
                  }
                  else {
                    uVar5 = (uint)*(byte *)(param_1 + 3);
                    cVar6 = 'A';
                  }
                  *(byte *)(param_1 + 0x12d) = (bVar4 & 0xe) - (char)((int)(uVar5 << 0x1c) >> 0x1f);
                  *(char *)(param_1 + 300) = cVar6;
                  *(undefined2 *)(param_1 + 0x128) = 2;
                  iVar2 = FUN_00009430(param_1);
                  if (iVar2 == 0) {
                    *(byte *)(param_1 + 0x1b) = *(byte *)(param_1 + 0x1b) | 1;
                  }
                }
                else if (*(char *)(param_1 + 0x29) == 'x') {
                  FUN_0000bf2c(param_1,0);
                }
                FUN_0004c888(*(undefined4 *)(param_1 + 0x10),8);
                if ((200 < (short)*(ushort *)(param_1 + 0x1de)) &&
                   (iVar1 = thunk_EXT_FUN_1fff8272
                                      (param_1,iVar1 + ((uint)*(ushort *)(iVar1 + 0x1658) -
                                                       (uint)*(ushort *)(param_1 + 0x1de)) * 2),
                   iVar1 == 0)) {
                  FUN_0000ecfc(param_1,pbVar7,0,*(undefined1 *)(param_1 + 0x124));
                }
              }
            }
            else {
              FUN_0000eef0(param_1,0,6,-iVar2 & 0xff);
            }
          }
          else if (iStack_4 == 4) {
            FUN_0000934e();
            FUN_0004c888(*(undefined4 *)(param_1 + 0x10),4);
          }
        }
        else {
          if (iStack_4 == 0x40) {
            FUN_0004cffc(2);
            uVar3 = 5;
          }
          else {
            if (iStack_4 != 0x80) goto LAB_0001ed7c;
            uVar3 = 6;
          }
          FUN_00009430(param_1,uVar3);
          uVar3 = *(undefined4 *)(param_1 + 0x10);
LAB_0001ee94:
          FUN_0004c888(uVar3,0x20);
        }
        goto LAB_0001ed7c;
      }
      if ((*(byte *)(param_1 + 0x18) & 6) == 0) {
        bVar4 = *(byte *)(param_1 + 0x1a) | 0x10;
LAB_0001ee90:
        uVar3 = *(undefined4 *)(param_1 + 0x10);
        *(byte *)(param_1 + 0x1a) = bVar4;
        goto LAB_0001ee94;
      }
      FUN_00009430(param_1,*(undefined1 *)(param_1 + 0x125));
      iVar1 = FUN_0004c7a0(0x20013ee4,0xffffffff,&iStack_4,3,5);
    } while (iVar1 == 0);
    *(byte *)(param_1 + 0x18) =
         (*(byte *)(param_1 + 0x18) & 0xf9) + (*(byte *)(param_1 + 0x18) + 6 & 6);
    FUN_0000eef0(param_1,1,6,9);
    FUN_0004c888(*(undefined4 *)(param_1 + 0x14),0x10);
  } while( true );
}
