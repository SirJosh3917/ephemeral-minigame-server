package com.sirn.proxy;

import net.md_5.bungee.api.ProxyServer;
import net.md_5.bungee.api.plugin.Plugin;

import java.net.InetAddress;
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
