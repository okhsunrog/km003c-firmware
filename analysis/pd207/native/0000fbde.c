/* WARNING: Globals starting with '_' overlap smaller symbols at the same address */

void FUN_0000fbde(void)

{
  uint uVar1;
  int iVar2;
  undefined1 *puVar3;
  undefined4 local_a0;
  undefined *local_9c;
  undefined4 local_94;
  undefined4 local_90;
  undefined4 local_8c;
  undefined1 local_88;
  undefined4 local_84;
  undefined4 local_80;
  undefined1 local_7c;
  undefined2 local_78;
  undefined2 uStack_76;
  undefined2 local_74;
  undefined1 uStack_72;
  undefined1 uStack_71;
  undefined4 local_70;
  undefined2 local_6c;
  undefined2 local_6a;
  undefined2 uStack_68;
  undefined2 local_65;
  undefined1 local_63;
  undefined2 local_62;
  undefined4 local_60;
  undefined4 local_5c;
  undefined4 local_58;
  undefined *local_54;
  undefined1 *local_50;
  undefined1 auStack_4c [24];
  undefined2 local_34;
  undefined1 local_2e;
  undefined1 local_2b;

  local_6a = 0;
  local_78 = 0;
  uStack_76 = 0;
  local_74 = 0;
  uStack_72 = 0;
  uStack_71 = 0;
  local_70 = 0;
  local_84 = 0;
  local_80 = 0;
  local_7c = 0;
  FUN_00009736(8,1);
  local_6c = 3;
  local_6a = CONCAT11(local_6a._1_1_,1);
  uStack_68 = 0xcd14;
  FUN_0000f12c(&DAT_40015400,&local_6c);
  local_78 = 0xcd14;
  uStack_76 = 0;
  local_74 = 0x302;
  uStack_72 = 0;
  FUN_0000f362(&DAT_40015400,3,&local_78);
  FUN_0000f362(&DAT_40015400,0,&local_78);
  FUN_0000f362(&DAT_40015400,1,&local_78);
  local_84 = CONCAT22(0x201,(undefined2)local_84);
  FUN_0000f1ee(&DAT_40015400,2,&local_84);
  local_88 = 0;
  local_90 = 0;
  local_94 = 0x10000;
  local_8c = 1;
  FUN_0000f5da(&DAT_40015400,&local_94);
  FUN_0000f7dc(0x1a0);
  FUN_0000f7cc(0x1a0);
  FUN_0000f528(&DAT_40015400,2,1);
  local_a0 = 0x80103;
  local_9c = &DAT_0000441f;
  FUN_0001dc2a(&local_a0);
  uVar1 = (uint)local_a0._2_2_;
  if (-1 < (int)uVar1) {
    *(int *)(&DAT_e000e280 + (uVar1 >> 5) * 4) = 1 << (uVar1 & 0x1f);
  }
  puVar3 = &DAT_e000e400 + uVar1;
  if ((int)uVar1 < 0) {
    puVar3 = (undefined1 *)((uVar1 & 0xf) + 0xe000ed14);
  }
  *puVar3 = 0xf0;
  uVar1 = (uint)local_a0._2_2_;
  if (-1 < (int)uVar1) {
    *(int *)(&DAT_e000e100 + (uVar1 >> 5) * 4) = 1 << (uVar1 & 0x1f);
  }
  local_62 = 0x120;
  FUN_0000974e(0x100,1);
  FUN_0000974e(0x10,1);
  FUN_00005c2e(0,&local_62);
  local_60 = 0x1010503;
  local_5c = (undefined1 *)((uint)local_5c._2_2_ << 0x10);
  FUN_00005c7e(&DAT_4004a000,&local_60);
  local_65 = 4;
  local_63 = 4;
  FUN_00005d44(&DAT_4004a000,&local_65);
  _DAT_4004a000 = _DAT_4004a000 | 0x8000;
  local_58 = &LAB_000201a0;
  local_54 = &DAT_0000441d;
  iVar2 = FUN_0001dc2a(&local_58);
  if (iVar2 == 0) {
    uVar1 = (uint)local_58._2_2_;
    if (-1 < (int)uVar1) {
      *(int *)(&DAT_e000e280 + (uVar1 >> 5) * 4) = 1 << (uVar1 & 0x1f);
    }
    puVar3 = &DAT_e000e400 + uVar1;
    if ((int)uVar1 < 0) {
      puVar3 = (undefined1 *)((uVar1 & 0xf) + 0xe000ed14);
    }
    *puVar3 = 0xa0;
    uVar1 = (uint)local_58._2_2_;
    if (-1 < (int)uVar1) {
      *(int *)(&DAT_e000e100 + (uVar1 >> 5) * 4) = 1 << (uVar1 & 0x1f);
    }
  }
  _DAT_42900060 = 0;
  _DAT_40048010 = 0xa5a50000;
  _DAT_40052000 = 0xb;
  _DAT_4005200c = 1;
  _DAT_40010804 = 0x105;
  FUN_000043fe(auStack_4c,0x24);
  _DAT_42900038 = 0;
  _DAT_42900044 = 0;
  _DAT_40048010 = 0xa5a50000;
  local_50 = &DAT_40015404;
  local_58 = (undefined1 *)0x1;
  local_54 = (undefined *)0x0;
  local_34 = 1;
  local_2e = 1;
  local_2b = 1;
  FUN_000063ae(&DAT_40053000,1,&local_58);
  local_34 = 0x100;
  local_54 = (undefined *)0x0;
  local_50 = (undefined1 *)0x0;
  local_2b = 0;
  FUN_000063ae(&DAT_40053000,2,&local_58);
  FUN_0000646c(&DAT_40053000,1,0x103);
  FUN_0000646c(&DAT_40053000,2,0x105);
  local_60 = 0x100021;
  local_5c = (undefined1 *)0xfb11;
  FUN_0001dc2a(&local_60);
  FUN_0001dc02(0x40);
  uVar1 = (uint)local_60._2_2_;
  if (-1 < (int)uVar1) {
    *(int *)(&DAT_e000e280 + (uVar1 >> 5) * 4) = 1 << (uVar1 & 0x1f);
  }
  puVar3 = &DAT_e000e400 + uVar1;
  if ((int)uVar1 < 0) {
    puVar3 = (undefined1 *)((uVar1 & 0xf) + 0xe000ed14);
  }
  *puVar3 = 0x50;
  uVar1 = (uint)local_60._2_2_;
  if (-1 < (int)uVar1) {
    *(int *)(&DAT_e000e100 + (uVar1 >> 5) * 4) = 1 << (uVar1 & 0x1f);
  }
  local_60 = 0x110022;
  local_5c = &LAB_0000fa6e_1;
  FUN_0001dc2a(&local_60);
  FUN_0001dc02(0x80);
  uVar1 = (uint)local_60._2_2_;
  if (-1 < (int)uVar1) {
    *(int *)(&DAT_e000e280 + (uVar1 >> 5) * 4) = 1 << (uVar1 & 0x1f);
  }
  puVar3 = &DAT_e000e400 + uVar1;
  if ((int)uVar1 < 0) {
    puVar3 = (undefined1 *)((uVar1 & 0xf) + 0xe000ed14);
  }
  *puVar3 = 0x50;
  uVar1 = (uint)local_60._2_2_;
  if (-1 < (int)uVar1) {
    *(int *)(&DAT_e000e100 + (uVar1 >> 5) * 4) = 1 << (uVar1 & 0x1f);
  }
  _DAT_40053000 = _DAT_40053000 | 1;
  return;
}
