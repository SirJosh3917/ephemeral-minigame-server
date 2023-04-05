package com.sirn.server;

import com.sirn.server.commands.CloseCommand;
import com.sirn.server.commands.RequestCommand;
import com.sirn.transport.ManagedControllerConnection;
import com.sirn.transport.packets.AuthenticationKind;
import com.sirn.transport.packets.AuthenticationPacket;
import com.sirn.transport.packets.AuthenticationPayload;

import org.bukkit.plugin.java.JavaPlugin;

import java.net.InetAddress;
import java.net.Socket;
import java.net.UnknownHostException;

public class ControllerPlugin extends JavaPlugin {
    @Override
    public void onEnable() {
        AuthenticationPacket authenticationPacket = new AuthenticationPacket();

        String ip = getServer().getIp();
        if (ip.length() == 0) {
            ip = "0.0.0.0";
        }
        authenticationPacket.ip = ip + ":" + getServer().getPort();

        AuthenticationKind authenticationKind = new AuthenticationKind();
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
            AuthenticationPayload authenticationPayload = new AuthenticationPayload();
            authenticationPayload.kind = minigameKind;
            authenticationKind.payload = authenticationPayload;
        }

        String controllerIp = System.getenv("CONTROLLER_IP");
        if (controllerIp == null) {
            getLogger().severe("Could not get `CONTROLLER_IP` from env vars.");
            return;
        }

		InetAddress address;
		try {
			address = InetAddress.getByName(controllerIp);
		} catch (UnknownHostException e) {
			getLogger().severe("Could not convert `CONTROLLER_IP` into an address (value: " + controllerIp + ")");;
			e.printStackTrace();
			return;
		}

		ServerPacketListener packetListener = new ServerPacketListener(this.getLogger(), authenticationPacket);

        getServer().getPluginCommand("request").setExecutor(new RequestCommand(packetListener));
        getServer().getPluginCommand("close").setExecutor(new CloseCommand(packetListener));

		new ManagedControllerConnection(this.getLogger(), () -> new Socket(address, 25550), packetListener);

        System.out.println("created head controller + cmds");
    }
}
