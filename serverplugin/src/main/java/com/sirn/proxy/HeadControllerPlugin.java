package com.sirn.proxy;

import net.md_5.bungee.api.ProxyServer;
import net.md_5.bungee.api.ReconnectHandler;
import net.md_5.bungee.api.config.ServerInfo;
import net.md_5.bungee.api.connection.ProxiedPlayer;
import net.md_5.bungee.api.plugin.Plugin;
import net.md_5.bungee.api.plugin.PluginDescription;
import net.md_5.bungee.config.Configuration;
import net.md_5.bungee.config.ConfigurationProvider;
import net.md_5.bungee.config.YamlConfiguration;

import java.io.File;
import java.io.IOException;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.net.UnknownHostException;

import com.sirn.transport.ManagedControllerConnection;

public class HeadControllerPlugin extends Plugin {
    @Override
    public void onEnable() {
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

		RejoinReconnectHandler reconnectHandler = new RejoinReconnectHandler(this.getLogger());
		this.getProxy().setReconnectHandler(reconnectHandler);

		ProxyPacketListener packetListener = new ProxyPacketListener(ProxyServer.getInstance(), reconnectHandler);
		new ManagedControllerConnection(this.getLogger(), () -> new Socket(address, 25550), packetListener);

        System.out.println("created head controller");
    }
}
