/* WARNING: Globals starting with '_' overlap smaller symbols at the same address */

undefined4 FUN_00009430(char *param_1,uint param_2)

{
  byte bVar1;
  undefined2 uVar2;
  undefined4 uVar3;
  int iVar4;
  bool bVar5;
  undefined1 auStack_1c [4];

  if ((int)((uint)(byte)param_1[0x18] << 0x1c) < 0) {
    iVar4 = -3;
    do {
      uVar3 = _DAT_2001a5d0;
      if ((*param_1 == '\x05') || ((_DAT_4004a004 & 1) != 0)) {
        uVar2 = thunk_EXT_FUN_1fff8184
                          (_DAT_2001a5d0,param_2,param_1 + 300,*(undefined2 *)(param_1 + 0x128));
        FUN_000052c0(0);
        FUN_0000fb46(0,0xfff3);
        FUN_00040958(param_1[0x21]);
        FUN_0000fa8c(uVar3,uVar2);
        iVar4 = FUN_0004c7a0(0x20013ee4,2,auStack_1c,0xff,5);
        FUN_0000f9c4();
        FUN_0004093e(param_1[0x21]);
        if (iVar4 != 0) {
          uVar3 = 9;
          goto LAB_00009520;
        }
        if (param_2 < 3) {
          FUN_0000ecfc(param_1,param_1 + 300,1,param_2);
LAB_00009506:
          uVar3 = 0;
        }
        else {
          if (param_2 == 5) {
            FUN_0000eef0(param_1,1,4,0);
            bVar1 = param_1[0x1a] | 8;
          }
          else {
            if (param_2 != 6) goto LAB_00009506;
            FUN_0000eef0(param_1,1,5,0);
            bVar1 = param_1[0x1a] | 0x80;
          }
          uVar3 = 0;
          param_1[0x1a] = bVar1;
        }
LAB_00009520:
        FUN_0000934e();
        FUN_000052c0(1);
        return uVar3;
      }
      FUN_0000657c(10);
      bVar5 = iVar4 != -1;
      iVar4 = iVar4 + 1;
    } while (bVar5);
    uVar3 = 9;
  }
  else {
    uVar3 = 10;
  }
  return uVar3;
}
