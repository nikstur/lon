{

  name = "lon";

  nodes = {
    machine = { };
    remote = {

    };
  };

  testScript = ''
    machine.wait_for_unit("multi-user.target")
    print(machine.succeed("lon-tests"));
  '';

}
