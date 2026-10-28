void FUN_00009268(int param_1)

{
  undefined4 unaff_r4;
  undefined4 unaff_lr;

  FUN_0004c734(0x20013ee4,0);
  FUN_0004ceb4(0x2001a7dc,s_PD_PHY_000092b8,0x1ed59,param_1,2,0x2001a5d8,0x200,0);
  if ((int)((uint)*(byte *)(param_1 + 0x26) << 0x1e) < 0) {
    return;
  }
  FUN_0000fbde();
  FUN_00040ab0(*(undefined1 *)(param_1 + 0x21));
  FUN_0000934e();
  FUN_0000eef0(param_1,1,1,*(undefined1 *)(param_1 + 0x21),unaff_r4,unaff_lr);
  *(byte *)(param_1 + 0x26) = *(byte *)(param_1 + 0x26) | 2;
  return;
}
