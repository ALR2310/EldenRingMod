# Danh sách ID TAE (animation) của Elden Ring

Tra cứu prefix `aNNN` của file animation TAE (`aNNN_xxxxxx`) theo loại vũ khí / phép / sword art - dùng khi cần biết 1 animation thuộc nhóm nào (vd. `SpeedMultiplier` lọc đòn đánh theo prefix).

Nguồn (gộp 2026-10-07 từ 2 file chép tay từ Nexus, đã bỏ 2 bản gốc): *Full TAE Named IDs List* (gốc, đầy đủ hơn: có khoảng `a000_xxxxxx`, tên field của param, các dòng `UNKNOWN`, phần cập nhật DLC 1.12) và *Elden Ring tae list updated for SOTE* (chỉ thêm bảng Idle/1H/2H bên dưới; ghi công "Syyyke's documentation"). Nội dung giữ nguyên bản gốc tiếng Anh.

Quy ước tên field: `wepmotionCategory`, `spAtkcategory`, `swordArtsTypeNew`, `swordArtsTypeDlc`, `refType` là cột tương ứng trong `EquipParamWeapon` / `SwordArtsParam` / `Magic`.

## Idle / Weapon Motion Position 1H-2H (`a000`–`a016`)

Theo bản SOTE (với sự trợ giúp của tài liệu của Syyyke):

```text
a000 - Straight sword, katana, staff...etc idle as well as emotes, rolls, deaths and every environnment interaction in the game
a002 - Greatsword, Curved greatswords, Great katanas Colossal swords/weapons
a003 - Spears, Halberds, Scythes
a010 - 2H Straight sword, katana, staff etc
a012 - 2H Greatsword, Curved greatswords, Great katanas Colossal swords/weapons
a013 - 2H Spears, Halberds, Scythes
a014 - Bow
a015 - Crossbow
a016 - 2H Seal
```

## Player motions chung (`a000`–`a015`)

Guard Motion Category (guardmotionCategory), Weapon Motion Position 1-H (wepmotionOneHandId) and Weapon Motion Position 2-H (wepmotionbothHandId) Specified in "EquipParamWeapon"

The following (a000-a015) hold many different variables of player motions including:

Staggers(against player)/Walk/Run/Sprint/Stopping/Turning/Backstep/Rolling/Quickstep/Climbing/Weapon Switch Transitions/Goods/Objects/Throws(against player)/Gestures/NPC Idles/Mount/Dismount/Jump/Crounch/Block

```text
a000_000000 = standing still
a000_000030-a000_000160 = holding block already up
a000_004000-a000_004121 = falling deaths
a000_004200-a000_004520 = falling landings
a000_004530-a000_004570 = lightning dash
a000_004900 = standing still
a000_005000-a000_005845 = staggers on player
a000_005850 = Frenzied
```

## Weapon Motion Category (`wepmotionCategory`) - EquipParamWeapon

```text
a020 - Dagger
a021 - Torch
a022 - Claw
a023 - Straight Sword
a024 - Twinblade
a025 - Greatsword
a026 - Colossal Sword
a027 - Thrusting Sword
a028 - Curved Sword
a029 - Katana
a030 - Axe
a031 - Colossal Weapon
a032 - Greataxe
a033 - Hammer
a034 - Flail
a035 - Great Hammer
a036 - Spear
a037 - Great Spear
a038 - Halberd
a039 - Heavy Thrusting Sword
a040 - Curved Greatsword
a041 - Catalyst
a042 - Fist
a043 - Whip
a044 - Bow
a045 - Greatbow
a046 - Crossbow
a047 - Greatshield
a048 - Small Shield
a049 - Medium Shield
a050 - Scythe
a051 - Light Bow
a052 - Ballista
[DLC ADDED 1.12]
a053 - Smithscript Dagger
a055 - Hand to Hand
a056 - Perfume Bottles
a057 - Thrust Shield
a058 - Backhand Blades
a060 - Light Greatsword
a061 - Great Katana
a062 - Beast Claw
```

## Special Motion Category (`spAtkcategory`) - EquipParamWeapon

```text
a100 - Great Knife/Ivory Sickle/Celebrant's Sickle
a101 - Misericorde/Scorpion's Stinger/Glintstone Kris
a103 - Erdsteel Dagger/Blade of Calling/Black Knife
a104 - Wakizashi
a110 - Broadsword/Cane Sword
a111 - Short Sword
a112 - Miquellan Knight's Sword
a113 - Carian Knight's Sword/Lazuli Glintstone Sword
a117 - Warhawk's Talon
a120 - Eleonora's Poleblade
a121 - Godskin Peeler
a125 - Claymore
a126 - Flamberge
a127 - Sword of Milos/Death's Poker
a128 - Knight's Greatsword/Banished Knight's Greatsword/Inseparable Sword
a129 - Dark Moon Greatsword
a130 - Marais Executioner's Sword
a135 - Zweihander/Troll Knight's Sword
a136 - Greatsword
a138 - Ruins Greatsword
a137 - Godslayer's Greatsword
a145 - Estoc/Noble's Estoc
a146 - Rogier's Rapier
a147 - Frozen Needle
a150 - Godskin Stitcher
a151 - Great epee
a155 - Shotel/Eclipse Shotel/Nox Flowing Sword
a156 - Scimitar/Shamshir
a157 - Flowing Curved Sword
a158 - Mantis Blade
a159 - Wing of Astel
a161 - Beastman's Curved Sword
a165 - Serpentbone Blade
a166 - Meteoric Ore Blade
a167 - Nagakiba
a170 - Hand Axe/Icerind Hatchet/Forked Hatchet
a171 - Warped Axe/Ripple Blade
a172 - Iron Cleaver/Celebrant's Cleaver
a175 - Butchering Knife
a176 - Pickaxe
a177 - Axe of Godrick/Crescent Moon Axe
a180 - Club/Stone Club
a181 - Spiked Club
a182 - Morning Star/Scepter of the All-Knowing
a183 - Mace
a184 - Monk's Flamemace
a196 - Prelate's Inferno Crozier
a195 - Great Club
a197 - Giant-Crusher
a198 - Golem's Halberd
a831 - Axe of Godfrey
a839 - Ghiza's Wheel
a200 - Partisan/Spiked Spear/Death Ritual Spear
a201 - Pike
a202 - Cross-Naginata
a203 - Short Spear/Cleanrot Spear
a205 - Vyke's War Spear/Siluria's Tree
a206 - Treespear
a207 - Serpent-Hunter
a210 - Halberd/Commander's Standard/Banished Knight's Halberd
a211 - Lucerne/Golden Halberd/Nightrider Glaive
a212 - Guardian's Swordspear/Loretta's War Sickle
a210 - Dragon Halberd
a225 - Scythe
a215 - Zamor Curved Sword
a216 - Omen Cleaver/Magma Wyrm's Scalesword
a220 - Katar/Veteran's Prosthesis
a221 - Caestus
a226 - Urumi
a227 - Raptor Talons
a230 - Albinauric Bow
a232 - Harp Bow
a233 - Pulley Crossbow
a236 - Full Moon Crossbow
a240 - Seal
a250 - Ghostflame Torch
a251 - St. Trina's Torch
a832 - Starscourge Greatsword/Starcaller Cry(swordArtsTypeNew)
a852 - Ornamental Straight Sword/Golden Tampering(swordArtsTypeNew)
[DLC ADDED 1.12]
a246 - Dane's Footwork
a247 - Smithscript Spear
a248 - Smithscript Axe
a249 - Rellana Twin Blades
a250 - Ghostflame Torch
a251 - Smithscript Cirque
a252 - Swift Spear
a253 - Black Steel Greathammer
a254 - Claws of Night
a255 - Falx/Horned Warrior's Sword
a256 - *UNKNOWN NOT SPECIFIED IN EQUIPPARAMWEAPON*
a257 - Dancing Blade of Ranah
a258 - Death Knight's Twin Axes
a259 - Golem Fist
a261 - Rabbath's Cannon
a262 - Main-Gauche
a263 - Fire Knights Greatsword
a264 - Lizard Greatsword
a265 - Curseblade's Cirque
a266 - Spear of the Impaler
a267 - Bloodfiend's Arm
a268 - Putrescence Cleaver
a288 - Nanaya's Torch (not in anibnd specified in hks)
a289 - Lamenting Visage (not in anibnd specified in hks)
a290 - Staff of the Great Beyond (not in anibnd specified in hks)
a291 - Ghostflame Torch (not in anibnd specified in hks)
a292 - St. Trina's Torch (not in anibnd specified in hks)
a293 - Carian Sorcery Sword (not in anibnd specified in hks)
a935 - Repeating Crossbow/Repeating Fire(IsDlcSwordArt)
a953 - Rakshasa's Great Katana/Weed Cutter(IsDlcSwordArt)
```

### Không được khai báo trong param

```text
a370
a371
a372
a373
a374
a375
a376
a377
a378
a379
a380
a381
```

## Magic Motion Category (`refType`) - Magic (TAE-400 = Motion Category)

```text
a400 - [Incantation] Inescapable Frenzy
a401 - [Sorcery] Pebble/Great Glintstone Shard/Glintstone Icecrag
a402 - [Sorcery] Glintstone Cometshard/Comet/Shard Spiral/Night Comet
a404 - [Sorcery] Crystal Barrage
a405 - [Sorcery] Loretta's Greatbow/Loretta's Mastery
a406 - [Sorcery] Rennala's Full Moon/Ranni's Dark Moon
a407 - [Sorcery] Comet Azur/Crystal Torrent
a408 - [Sorcery] Glintblade Phalanx/Carian Phalanx/Eternal Darkness
a409 - [Sorcery] Carian Greatsword/Adula's Moonblade
a410 - [Sorcery] Carian Piercer
a411 - [Sorcery] Scholar's Armament/Unseen Blade
a412 - [Sorcery] Scholar's Shield
a413 - [Sorcery] Terra Magicus/Starlight/Lucidity/Frozen Armament
a414 - [Sorcery] Zamor Ice Storm
a415 - [Sorcery] Meteorite/Meteorite of Aste
a416 - [Sorcery] Glintstone Arc
a417 - [Incantation] Flame Sling/Flame, Fall Upon Them/Giantsflame Take Thee/Black Flame
a419 - [Incantation] Flame of the Fell God
a420 - [Incantation] Whirl, O Flame!
a422 - [Incantation] Scouring Black Flame
a423 - [Incantation] Surge, O Flame!
a424 - [Incantation] Burn, O Flame!/Fire's Deadly Sin
a425 - [Incantation] O, Flame!
a426 - [Incantation] Shadow Bait/Darkness/Swarm of Flies/Poison Mist
a427 - [Incantation] Discus of Light/Triple Rings of Light
a428 - [Incantation] Rejection
a429 - [Incantation] Wrath of Gold
a431 - [Incantation] Flame, Cleanse Me/Flame, Grant Me Strength/Flame, Protect Me/Black Flame's Protection/Bestial Vitality/Bestial Constitution
a432 - [Incantation] Black Flame Blade
a433 - [Incantation] Urgent Heal/Cure Poison/Flame Fortification/Magic Fortification/Lightning Fortification/Divine Fortification
a434 - [Incantation] Barrier of Gold/Protection of the Erdtree/Heal/Great Heal/Lord's Heal/Erdtree Heal/Blessing's Boon/Blessing of the Erdtree/Lord's Aid/Lord's Divine Fortification/Golden Lightning Fortification
a435 - [Incantation] Golden Vow
a436 - [Incantation] Lightning Spear
a437 - [Incantation] Stone of Gurranq
a438 - [Incantation] Bestial Sling
a440 - [Incantation] Beast Claw
a441 - [Incantation] Gurranq's Beast Claw
a442 - [Incantation] Death Lightning
a444 - [Incantation] Ancient Dragons' Lightning Spear
a445 - [Incantation] Lansseax's Glaive
a448 - [Incantation] Radagon's Rings of Light
a449 - [Incantation] Immutable Shield
a450 - [Incantation] Agheel's Flame/Borealis's Mist/Ekzykes's Decay/Smarag's Glintstone Breath
a451 - [Incantation] Placidusax's Ruin
a452 - [Incantation] Dragonclaw
a454 - [Incantation] Dragonmaw
a455 - [Incantation] Greyoll's Roar
a456 - [Sorcery] Shatter Earth
a457 - [Sorcery] Rock Blaster
a458 - [Sorcery] Crystal Release
a459 - [Incantation] Electrify Armament/Vyke's Dragonbolt
a460 - [Incantation] Dragonbolt Blessing
a466 - [Incantation] The Flame of Frenzy
a467 - [Incantation] Dragonfire/Dragonice/Rotten Breath/Glintstone Breath
a470 - [Incantation] Aspects of the Crucible: Tail
a471 - [Incantation] Aspects of the Crucible: Horns
a474 - [Incantation] Pest Threads
a475 - [Sorcery] Oracle Bubbles
a476 - [Sorcery] Great Oracular Bubble
a478 - [Sorcery] Gavel of Haima
a479 - [Sorcery] Swift Glintstone Shard/Night Shard
a480 - [Sorcery] Carian Slicer
a481 - [Sorcery] Briars of Sin
a482 - [Sorcery] Briars of Punishment
a483 - [Sorcery] Ambush Shard
a484 - [Sorcery] Carian Retaliation
a486 - [Incantation] Law of Regression/Law of Causality/Order Healing
a487 - [Incantation] Litany of Proper Death
a488 - [Incantation] Unendurable Frenzy
a489 - [Incantation] Frenzied Burst
a490 - [Incantation] Howl of Shabriri
a491 - [Incantation] Elden Stars
a492 - [Sorcery] Cannon of Haima
a493 - [Incantation] Bloodboon
a494 - [Incantation] Bloodflame Talons
a495 - [Incantation] Greatblade Phalanx/Rykard's Rancor
a498 - [Incantation] Fortissax's Lightning Spear
a499 - [Incantation] Black Flame Ritual
a500 - [Incantation] Aspects of the Crucible: Breath
a505 - [Incantation] Noble Presence
a502 - [Sorcery] Gelmir's Fury
a503 - [Sorcery] Rancorcall/Ancient Death Rancor
a504 - [Incantation] Order's Blade
a506 - [Incantation] Catch Flame
a507 - [Sorcery] Glintstone Stars/Star Shower/Stars of Ruin/Magic Downpour
a508 - [Sorcery] Rock Sling
a509 - [Sorcery] Unseen Form
a510 - [Sorcery] Thops's Barrier
a511 - [Incantation] Honed Bolt
a512 - [Incantation] Bloodflame Blade/Poison Armament
a513 - [Sorcery] Crystal Burst/Freezing Mist/Shattering Crystal/Fia's Mist/Night Maiden's Mist
a514 - [Sorcery] Assassin's Approach
a515 - [Sorcery] Explosive Ghostflame
a516 - [Incantation] Black Blade
a517 - [Sorcery] Magma Shot/Roiling Magma
a518 - [Sorcery] Magic Glintblade
a519 - [Incantation] Frozen Lightning Spear
a520 - [Sorcery] Gravity Well/Collapsing Stars
a521 - [Sorcery] Tibia's Summons
a522 - [Incantation] Scarlet Aeonia
a523 - [Sorcery] Magma Breath/Theodorix's Magma
a524 - [Sorcery] Founding Rain of Stars
[DLC ADDED 1.12]
a525 - [Incantation] Aspects of the Crucible: Thorns
a526 - [Sorcery] Vortex of Putrescence
a527 - [Sorcery] Miriam's Vanishing
a528 - [Incantation] Minor Erdtree
a529 - [Incantation] Aspects of the Crucible: Bloom
a531 - [Incantation] Roar of Rugalea
a533 - [Incantation] Bayle's Tyranny
a534 - [Incantation] Bayle's Flame Lightning
a535 - [Incantation] Rotten Butterflies
a536 - [Incantation] Pest-Thread Spears
a537 - [Incantation] Midra's Flame of Frenzy
a538 - [Sorcery] Glintblade Trio
a539 - [Incantation] Knight's Lightning Spear
a540 - [Incantation] Furious Blade of Ansbach
a541 - [Incantation] Messmer's Orb
a542 - *UNKNOWN ROW LINE (2050064)*
a543 - [Sorcery] Rings of Spectral Light
a544 - [Incantation] Dragonbolt of Florissax
a545 - [Incantation] Light of Miquella
a546 - [Sorcery] Spira
a547 - [Incantation] Divine Beast Tornado
a548 - [Sorcery] Golden Arcs
a549 - [Incantation] Multilayered Ring of Light
a550 - [Sorcery] Mantle of Thorns
a551 - [Incantation] Wrath from Afar
a552 - [Incantation] Divine Bird Feathers
a553 - [Incantation] Fire Serpent
a554 - [Sorcery] Giant Golden Arc
a556 - [Sorcery] Impenetrable Thorns
a557 - [Sorcery] Cherishing Fingers
a558 - [Sorcery] Mass of Putrescence
a559 - [Incantation] Rain of Fire
a560 - *UNKNOWN ROW LINE (2050063)*
a561 - [Sorcery] Rellana's Twin Moons
a562 - *UNKNOWN ROW LINE (2050010)*
```

## Sword Arts Type (`swordArtsTypeNew`) - SwordArtsParam (TAE-600 = Sword Arts Type)

```text
a600 - Lion's Claw
a601 - Impaling Thrust
a602 - Piercing Fang
a603 - Spinning Slash
a605 - Charge Forth
a606 - Stamp (Upward Cut)
a607 - Stamp (Sweep)
a608 - Blood Tax
a609 - Repeating Thrust
a610 - Wild Strikes
a611 - Spinning Strikes
a612 - Double Slash
a613 - Prelate's Charge
a614 - Unsheathe
a615 - Square Off
a616 - Giant Hunt
a617 - Torch Attack
a618 - Loretta's Slash
a619 - Poison Moth Flight
a620 - Spinning Weapon
a622 - Storm Assault
a623 - Stormcaller
a624 - Sword Dance
a625 - Spinning Chain
a650 - Glintblade Phalanx
a651 - Sacred Blade
a652 - Ice Spear
a653 - Glintstone Pebble
a654 - Bloody Slash
a655 - Lifesteal Fist
a656 - Eruption
a657 - Prayerful Strike
a658 - Gravitas
a659 - Storm Blade
a661 - Earthshaker
a662 - Golden Land
a663 - Flaming Strike
a664 - Thunderbolt
a665 - Lightning Slash
a666 - Carian Grandeur
a667 - Carian Greatsword
a668 - Vacuum Slice
a669 - Black Flame Tornado
a670 - Sacred Ring of Light
a671 - Firebreather
a672 - Blood Blade
a673 - Phantom Slash
a674 - Spectral Lance
a675 - Chilling Mist
a676 - Poisonous Mist
a690 - Shield Bash
a691 - Barricade Shield
a692 - Parry
a693 - Buckler Parry
a695 - Carian Retaliation
a696 - Storm Wall
a697 - Golden Parry
a698 - Shield Crash
a699 - Thops's Barrier
a700 - Through and Through
a701 - Barrage
a702 - Mighty Shot
a703 - Enchanted Shot
a708 - Sky Shot
a705 - Rain of Arrows
a710 - Hoarfrost Stomp
a711 - Storm Stomp
a712 - Kick
a713 - Lightning Ram
a714 - Flame of the Redmanes
a715 - Ground Slam
a716 - Golden Slam
a717 - Waves of Darkness
a718 - Hoarah Loux's Earthshaker
a730 - Determination
a731 - Royal Knight's Resolve
a732 - Assassin's Gambit
a733 - Golden Vow
a734 - Sacred Order
a735 - Shared Order
a736 - Seppuku
a737 - Cragblade
a740 - Barbaric Roar
a741 - War Cry
a742 - Beast's Roar
a743 - Troll's Roar
a744 - Braggart's Roar
a750 - Endure
a751 - Vow of the Indomitable
a752 - Holy Ground
a755 - Quickstep
a756 - Bloodhound's Step
a757 - Raptor of the Mists
a760 - White Shadow's Lure
a767 - Corpse Wax Cutter
a768 - Zamor Ice Storm
a769 - Radahn's Rain
a770 - The Queen's Black Flame
a771 - Dynast's Finesse
a772 - Magma Shower
a773 - Nebula
a774 - Death Flare
a775 - Bloodhound's Finesse
a776 - Magma Guillotine
a777 - Corpse Piler
a778 - Transient Moonlight
a779 - Bloodblade Dance
a782 - Knowledge Above All
a783 - Devourer of Worlds
a784 - Familal Rancor
a785 - Rosus's Summons
a786 - Thunderstorm
a787 - Sacred Phalanx
a788 - Great-Serpent Hunt
a789 - Angel's Wings
a790 - Storm Kick
a791 - Unblockable Blade
a792 - Sorcery of the Crozier
a793 - Erdtree Slam
a794 - Gravity Bolt
a795 - Fires of Slumber
a796 - Golden Retaliation
a797 - Contagious Fury
a798 - Ordovis's Vortex
a799 - Spinning Weapon
a800 - Surge of Faith
a801 - Flame Spit
a802 - Tongues of Fire
a803 - Oracular Bubble
a804 - Bubble Shower
a805 - Great Oracular Bubble
a806 - Sea of Magma
a807 - Viper Bite
a808 - Moonlight Greatsword
a809 - Siluria's Woe
a810 - Rallying Standard
a811 - Bear Witness!
a812 - Eochaid's Dancing Blade
a813 - Soul Stifler
a814 - Taker's Flames
a815 - Shriek of Milos
a816 - Reduvia Blood Blade
a817 - Glintstone Dart
a818 - Flowing Form
a819 - Night-and-Flame Stance
a820 - Wave of Gold
a821 - Ruinous Ghostflame
a822 - Establish Order
a823 - Mists of Slumber
a824 - Spearcall Ritual
a825 - Wolf's Assault
a826 - Thundercloud Form
a827 - Cursed-Blood Slice
a828 - Waterfowl Dance
a829 - Gold Breaker
a830 - I Command Thee, Kneel!
a831 - Regal Roar
a832 - Starcaller Cry/Starscourge Greatsword(spAtkcategory)
a833 - Wave of Destruction
a834 - Bloodboon Ritual
a835 - Flowing Form
a836 - Blade of Death
a837 - Blade of Gold
a838 - Destined Death
a839 - Spinning Wheel
a840 - Alabaster Lords' Pull
a841 - Onyx Lords' Repulsion
a842 - Oath of Vengeance
a843 - Ice Lightning Sword
a844 - Regal Beastclaw
a845 - Flame Dance
a846 - Claw Flick
a847 - Nebula
a848 - Ghostflame Ignition
a849 - Ancient Lightning Spear
a850 - Frenzyflame Thrust
a851 - Miquella's Ring of Light
a852 - Golden Tempering/Ornemental Straight Sword(spAtkcategory)
a853 - Last Rites
a854 - Unblockable Blade
```

## Sword Arts Type DLC (`swordArtsTypeDlc`) - TAE-600 = Sword Arts Type

```text
a856 - Aspect of the Crucible: Wings
a858 - Dryleaf Whirlwind
a860 - Spinning Gravity Thrust
a861 - Palm Blast
a862 - Piercing Throw
a863 - Scattershot Throw
a864 - Wall of Sparks
a865 - Rolling Sparks
a869 - Painful Strike
a872 - Hone Blade
a873 - Raging Beast
a874 - Savage Claws
a875 - Red Bear Hunt
a876 - Blind Spot
a877 - Swift Slash
a878 - Overhead Stance
a879 - Wing Stance
a880 - Blackbolt
a881 - Flame Skewer
a882 - Savage Lion's Claw
a883 - Divine Beast Frost Stomp
a884 - Flame Spear
a885 - Carian Sovereignty
a886 - Shriek of Sorrow
a900 - Dragonwound Slash
a901 - Needle Piercer
a902 - Light
a903 - Darkness
a904 - Onze's Line of Stars
a905 - The Poison Flower Blooms Twice
a908 - Spinning Guillotine
a909 - Unending Dance
a910 - Revenger's Blade
a911 - Mists of Eternal Sleep
a913 - Dynastic Sickleplay
a914 - Blinkbolt: Twinaxe
a915 - Blinkbolt: Long-hafted Axe
a916 - Promised Consort
a917 - Shadow Sunflower Headbutt
a918 - Moon-and-Fire Stance
a919 - Devonia's Vortex
a920 - Messmer's Assault
a922 - Sleep Evermore
a923 - Golden Crux
a924 - Moore's Charge
a925 - White Light Charge
a927 - Witching Hour Slash
a928 - Euporia Vortex
a929 - Smithing Art Spears
a931 - Romina's Purification
a932 - Poison Spear-Hand Strike
a933 - Madding Spear-Hand Strike
a934 - Feeble Lord's Frenzied Flame
a935 - Repeating Fire/Repeating Crossbow(spAtkcategory)
a936 - Deadly Dance
a937 - Fan Shot
a948 - Discus Hurl
a949 - Flower Dragonbolt
a950 - Kowtower's Resentment
a951 - Solitary Moon Slash
a952 - Revenge of the Night
a953 - Weed Cutter/Rakshasa's Great Katana(spAtkcategory)
a954 - Blindfold of Happiness
a955 - UNKNOWN ROW NAME (5461)
a956 - Jori's Inquisition
a957 - Igon's Drake Hunt
a958 - Deadly Poison Spray
a959 - Roaring Bash
a960 - Flare, O Serpent
a961 - Scattershot (Claws)
a962 - Horn Calling
a963 - Horn Calling: Storm
a964 - Ghostflame Call
a965 - Rancor Slash
a967 - Tremendous Phalanx
a968 - Bloodfiends' Bloodboon
a969 - Dragonform Flame
a970 - Lightspeed Slash
a971 - Rancor Shot
```
