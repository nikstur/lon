{

  name = "lon";

  nodes = {
    remote =
      { pkgs, ... }:
      let
        gitRepo =
          let
            branch = "myBranch";
            tag = "23.42.69";
          in
          pkgs.runCommand "git-repo" { nativeBuildInputs = [ pkgs.gitMinimal ]; } ''
            export HOME=$TMP
            git config --global user.email "you@example.com"
            git config --global user.name "Your Name"
            git config --global init.defaultBranch main

            mkdir tmp
            git init tmp
            cd tmp

            touch init.txt
            git add init.txt
            git commit -v -m "init"

            git checkout -B '${branch}'
            touch test.txt
            git add test.txt
            git commit -v -m "test"

            echo '${tag}' > test.txt
            git add test.txt
            git commit -v -m 'commit for tag ${tag}'
            git tag '${tag}'

            git update-server-info
            cp -r .git $out
          '';
      in
      {
        networking.firewall.enable = false;
        services = {
          openssh.enable = true;
          nginx = {
            enable = true;
            virtualHosts.default = {
              locations."/repo.git/".alias = "${gitRepo}/";
            };
          };
        };

        users.users.git = {
          isNormalUser = true;
          packages = [ pkgs.git ];
          openssh.authorizedKeys.keys = [
            (builtins.readFile ./fixtures/key.pub)
          ];
        };

        systemd.tmpfiles.settings."10-git" = {
          "/home/git/repo.git".C.argument = toString gitRepo;
          "/home/git/repo.git".Z.user = "git";
        };
      };
    machine =
      { pkgs, ... }:
      {
        programs.ssh.extraConfig = ''
          Host *
           StrictHostKeyChecking accept-new
        '';

        environment.systemPackages = [ pkgs.git ];

        systemd.tmpfiles.settings."10-lon" = {
          "/root/.ssh/id_ed25519".C.argument = "${./fixtures/key}";
          "/root/.ssh/id_ed25519".z.mode = "0600";
        };
      };
  };

  testScript = ''
    start_all()
    remote.wait_for_unit("multi-user.target")
    machine.wait_for_unit("multi-user.target")

    with subtest("Ensure basic accessibility"):
      print(machine.succeed("GIT_TRACE=true GIT_SSH_COMMAND='ssh -vvv' git clone git@remote:repo.git ssh"));
      print(machine.succeed("git clone http://remote/repo.git http"));

    print(machine.succeed("lon-tests"));
  '';

}
