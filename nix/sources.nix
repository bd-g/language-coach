# Pinned Nix inputs. Update a pin by changing `rev` and replacing `sha256` with
# the output of `nix-prefetch-url --unpack <url>`.

let
  github =
    {
      owner,
      repo,
      rev,
      sha256,
    }:
    builtins.fetchTarball {
      url = "https://github.com/${owner}/${repo}/archive/${rev}.tar.gz";
      inherit sha256;
    };
in
{
  nixpkgs = github {
    owner = "NixOS";
    repo = "nixpkgs";
    rev = "04906337b6d53e8bb0eaf0470ae9f070a4d4eb55";
    sha256 = "1ixmpdkxv55py03w9gj4qn4p90jn494a5zzx3j90c703gaa3dhcl";
  };

  pyproject-nix = github {
    owner = "pyproject-nix";
    repo = "pyproject.nix";
    rev = "d43f0e74444c1f266c640fc49ce6ce6fb69d33c3";
    sha256 = "1d5qx0skdpwzrpg8hjfc11spwjjw9j8gd36mhw0gsbysgdp1lhrm";
  };

  uv2nix = github {
    owner = "pyproject-nix";
    repo = "uv2nix";
    rev = "c361746c7d630d98b972378290547c8d35542a7a";
    sha256 = "08xh8gsz4r6pxri2px657399r9w6wiq92mal8r3qkb999h43a8fw";
  };

  pyproject-build-systems = github {
    owner = "pyproject-nix";
    repo = "build-system-pkgs";
    rev = "bdbc5f8dd2ad6136bb2ccecb14f6ec343965b194";
    sha256 = "1n4rq6pszzhs6jc9lcgrc2i4sqzr76i110nz6qln047d1r78a4nc";
  };
}
