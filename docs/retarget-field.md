# retarget-field

Access a native property the compiler rejects.

## 1. Stand-in script

```witcherscript
@addField(CLightComponent) var lrLightUsageMaskProxy: int;

@addMethod(CLightComponent)
public function LR_GetLightUsageMask(): int {
    return lrLightUsageMaskProxy;
}

@addMethod(CLightComponent)
public function LR_SetLightUsageMask(mask: int) {
    lrLightUsageMaskProxy = mask;
}
```

## 2. Compile

Output: `lr-precompiled2.rsblob`

## 3. Build tool

```sh
RUSTC_BOOTSTRAP=1 cargo build --release
```

## 4. Patch

```sh
./target/release/tw3-cahirb.exe retarget-field lr-precompiled2.rsblob lr-precompiled2.patched.rsblob lrLightUsageMaskProxy lightUsageMask
```

## 5. Verify

```sh
./target/release/tw3-cahirb.exe roundtrip lr-precompiled2.patched.rsblob
./target/release/tw3-cahirb.exe diff lr-precompiled2.rsblob lr-precompiled2.patched.rsblob
```

Expected diff:

```
## missing in actual (1)
- @addField CLightComponent::lrLightUsageMaskProxy
## changed definitions (2)
### @addMethod CLightComponent::LR_GetLightUsageMask
-  ObjectVar property=CLightComponent.lrLightUsageMaskProxy
+  ObjectVar property=CLightComponent.lightUsageMask
### @addMethod CLightComponent::LR_SetLightUsageMask
-  ObjectVar property=CLightComponent.lrLightUsageMaskProxy
+  ObjectVar property=CLightComponent.lightUsageMask
## tables (1)
- string table: 1 missing, 1 extra, first missing "lrLightUsageMaskProxy", first extra "lightUsageMask"
```

## Constraints

- Proxy name: unique, used only by the stand-in field
- Native name: absent from string table
- Proxy type: same size as native type
