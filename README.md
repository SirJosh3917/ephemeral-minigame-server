# Ephemeral Minigame Server

<p align="center">
	<a href="https://youtu.be/1MKlHS_CmA8">
		<img src="https://i.imgur.com/Pw4K7G2.png" />
	</a>
</p>

This code is a prototypal implementation of a minigame franchise server. In
short, there are a few components:

- **Controller**: the brains of the operation, orchestrating and communicating
  between the various ephemeral servers.
- **Proxy**: receives incoming player connections, and teleports them to
  different servers.
- **Lobby servers**: ephemeral, dynamically scalable servers that proxies
  forward players to upon joining.
- **Minigame servers**: ephemeral, dynamically scalable servers that players
  must ask to join.

This could eventually be horizontally scaled if you put in time to look into
Docker Swarm.

## Try it!

If you have Docker, this project should be a piece of cake to deploy. Try it
out!

0. Clone the project

   ```sh
   git clone https://github.com/SirJosh3917/ephemeral-minigame-server.git
   cd ephemeral-minigame-server
   ```

1. Download some plugins

   Put these jar files into `./constructor/assets/waterfall/`. Pretty sure I
   can't automate this for ToS reasons.

   - [ViaBackwards](https://www.spigotmc.org/resources/viabackwards.27448/)
   - [ViaRewind](https://www.spigotmc.org/resources/viarewind.52109/)
   - [ViaVersion](https://www.spigotmc.org/resources/viaversion.19254/)

2. Agree to the EULA

   Agree to the EULA in `./constructor/eula.txt`. Also pretty sure I can't
   automate this.

   ```sh
   # Agree to Mojang's EULA.
   echo "eula=true" >> constructor/eula.txt
   ```

3. Start the project!

   ```sh
   # Necessary wrapper for `docker compose up`
   ./start.sh
   ```

   If you're on windows, the script is simple enough - you can figure it out :)

It might take a long while before it starts up, but soon you'll have a server
listening on `:25565` to join!
