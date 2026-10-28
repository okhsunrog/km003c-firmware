void FUN_00040958(int param_1)

{
  undefined4 *puVar1;
  undefined4 *puVar2;

  puVar2 = (undefined4 *)&DAT_42a704b8;
  if (param_1 == 1) {
    puVar1 = (undefined4 *)&DAT_42a704f8;
  }
  else {
    if (param_1 != 2) {
      return;
    }
    puVar1 = (undefined4 *)&DAT_42a704fc;
    puVar2 = (undefined4 *)&DAT_42a704bc;
  }
  *puVar2 = 0;
  *puVar1 = 1;
  return;
}
