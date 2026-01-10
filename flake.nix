{
  description = "Sharing home-manager modules between nixos and darwin";

  # Add all your dependencies here
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    blueprint = {
      url = "github:numtide/blueprint";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    home-manager = {
      url = "github:nix-community/home-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    hyprland.url = "github:hyprwm/hyprland";

    zen-browser = {
      url = "github:0xc000022070/zen-browser-flake/beta";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        home-manager.follows = "home-manager";
      };
    };

    gitalias = {
      url = "github:GitAlias/gitalias";
      flake = false;
    };

    nixpkgs-zed.url = "github:NixOS/nixpkgs/nixos-25.11";
  };

  # Load blueprint
  outputs =
    inputs:
    inputs.blueprint {
      inherit inputs;
      nixpkgs.config.allowUnfree = true;
    };
}
