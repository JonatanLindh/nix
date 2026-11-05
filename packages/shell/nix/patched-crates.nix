{ applyPatches, fetchCrate }:

# gpui-pre with patches/ applied, for Cargo's [patch.crates-io] to point at through
# .patched/<crate> (linked by devenv's enterShell, and by the package's postPatch). Drop a crate
# here once its patch is upstream.
let
  patchedCrate =
    pname: version: hash:
    applyPatches {
      name = "${pname}-${version}-patched";
      src = fetchCrate { inherit pname version hash; };
      patches = [ ../patches/${pname}+${version}.patch ];
    };
in
{
  gpui-pre = patchedCrate "gpui-pre" "0.3.11" "sha256-ILF1poFhTcQ4GvwkHM/eWhF06ZTzjCaiekkHYUla90c=";
  gpui-pre-linux =
    patchedCrate "gpui-pre-linux" "0.3.11"
      "sha256-0D0CuwxSUvIqQK/GoyHUqQNa/XdWelJU0bzUKh7ChX0=";
}
