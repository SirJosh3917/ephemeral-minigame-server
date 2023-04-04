package com.sirn;

import com.sirn.commands.CloseCommand;
import com.sirn.commands.RequestCommand;
import com.sirn.controller_connection.HeadController;
import com.sirn.controller_connection.packets.ServerKind;
import com.sirn.controller_connection.packets.AuthenticationPacket;
import com.sirn.controller_connection.packets.MinigamePayload;
import org.bukkit.plugin.java.JavaPlugin;

import java.io.IOException;
import java.net.Socket;

public class ControllerPlugin extends JavaPlugin {
    @Override
    public void onEnable() {
        AuthenticationPacket authenticationPacket = new AuthenticationPacket();

        String ip = getServer().getIp();
        if (ip.length() == 0) {
            ip = "0.0.0.0";
        }
        authenticationPacket.ip = ip + ":" + getServer().getPort();

        ServerKind authenticationKind = new ServerKind();
        authenticationPacket.kind = authenticationKind;

        String serverName = System.getenv("SERVER_NAME");
        if (serverName == null) {
            getLogger().severe("Could not get `SERVER_NAME` from env vars.");
            return;
        }
        authenticationPacket.name = serverName;

        String serverKind = System.getenv("SERVER_KIND");
        if (serverKind == null) {
            getLogger().severe("Could not get `SERVER_KIND` from env vars.");
            return;
        }
        authenticationKind.tag = serverKind;

        if (serverKind.equals("Minigame")) {
            String minigameKind = System.getenv("MINIGAME_KIND");
            if (minigameKind == null) {
                getLogger().severe("Could not get `MINIGAME_KIND` from env vars.");
                return;
            }
            MinigamePayload authenticationPayload = new MinigamePayload();
            authenticationPayload.kind = minigameKind;
            authenticationKind.payload = authenticationPayload;
        }

        Socket socket = null;

        String controllerIp = System.getenv("CONTROLLER_IP");
        if (controllerIp == null) {
            getLogger().severe("Could not get `CONTROLLER_IP` from env vars.");
            return;
        }

        try {
            socket = new Socket(controllerIp, 25550);
        } catch (IOException e) {
            e.printStackTrace();
            System.out.println("couldn't make socket");
            return;
        }

        try {
            HeadController headController = new HeadController(this.getLogger(), authenticationPacket, socket);
            getServer().getPluginCommand("request").setExecutor(new RequestCommand(headController));
            getServer().getPluginCommand("close").setExecutor(new CloseCommand(headController));
        } catch (IOException e) {
            e.printStackTrace();
            return;
        }

        System.out.println("created head controller + cmds");
    }
}
