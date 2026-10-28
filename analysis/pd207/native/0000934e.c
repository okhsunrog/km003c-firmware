/* WARNING: Globals starting with '_' overlap smaller symbols at the same address */

void FUN_0000934e(void)

{
  undefined4 *puVar1;
  undefined1 *puVar2;
  undefined4 *puVar3;

  puVar2 = (undefined1 *)_DAT_2001a5d0;
  puVar3 = (undefined4 *)&DAT_200111e8;
  if (_DAT_2001a5d0 == (undefined4 *)&DAT_200111e8) {
    puVar3 = (undefined4 *)&DAT_20012866;
  }
  _DAT_2001a5d0 = puVar3;
  *puVar3 = 0;
  puVar1 = (undefined4 *)&DAT_200111e8;
  if (puVar2 == &DAT_200111e8) {
    puVar1 = (undefined4 *)&DAT_20012866;
  }
  *(undefined2 *)(puVar1 + 0x596) = 0;
  FUN_0000fa02(puVar3,0xb2c,0x98);
  FUN_0000fb46(1,&LAB_0000d246_1);
  return;
}
