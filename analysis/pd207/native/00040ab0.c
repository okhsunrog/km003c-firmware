void FUN_00040ab0(int param_1)

{
  undefined4 uVar1;
  undefined4 uVar2;
  undefined4 uVar3;

  if (param_1 == 2) {
    uVar1 = 2;
    uVar2 = 1;
    uVar3 = 2;
  }
  else {
    if (param_1 != 1) {
      return;
    }
    uVar1 = 1;
    uVar2 = 0;
    uVar3 = 1;
  }
  FUN_0000f976(uVar1,uVar2,0);
  FUN_0000f86e(uVar3,2,0);
  return;
}
