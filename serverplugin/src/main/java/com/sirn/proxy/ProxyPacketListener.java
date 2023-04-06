package com.sirn.proxy;

import java.io.IOException;
import java.net.InetSocketAddress;
import java.util.*;
import java.util.logging.Logger;

import com.sirn.transport.ControllerConnection;
import com.sirn.transport.ControllerEventListener;
import com.sirn.transport.packets.*;

import net.md_5.bungee.api.ProxyServer;
import net.md_5.bungee.api.config.ListenerInfo;
import net.md_5.bungee.api.config.ServerInfo;
import net.md_5.bungee.api.connection.ProxiedPlayer;

public class ProxyPacketListener extends ControllerEventListener {
	private final Logger logger;
	private final ProxyServer proxyServer;
	// For some reason, I employed levels of priority for servers.
	// No idea why I did that in retrospect, but too lazy to change atm.
	private final TreeMap<Integer, List<String>> serverPriorities;
	public final RejoinReconnectHandler rejoinReconnectHandler;

	public ProxyPacketListener(ProxyServer proxyServer, RejoinReconnectHandler reconnectHandler) {
		this.proxyServer = proxyServer;
		this.logger = this.proxyServer.getLogger();
		this.serverPriorities = new TreeMap<>();
		this.rejoinReconnectHandler = reconnectHandler;
	}

	private ControllerConnection connection;

	@Override
	public void onConnect(ControllerConnection connection) throws IOException {
		this.connection = connection;

		AuthenticationKind authenticationKind = new AuthenticationKind();
		authenticationKind.tag = "Proxy";

		AuthenticationPacket authenticationPacket = new AuthenticationPacket();
		authenticationPacket.name = "proxy";
		authenticationPacket.kind = authenticationKind;

		ListenerInfo listenerInfo = this.proxyServer.getConfig().getListeners().iterator().next();
		assert listenerInfo != null;
		authenticationPacket.ip = listenerInfo.getSocketAddress().toString();

		this.connection.write(authenticationPacket);
		this.logger.info("Authentication packet sent");
	}

	@Override
	public void onDisconnect() {
		this.logger.info("Proxy disconnected from controller!");
		this.logger.info("TODO: Implement more robust logic to repair things upon disconnecting");
	}

	@Override
	public void onLinkServerPacket(LinkServerPacket packet) {
		this.logger.info("Linking connected server " + packet);

		// Add server to the proxy
		ServerInfo linkedServerInfo = this.proxyServer.constructServerInfo(
				packet.name,
				new InetSocketAddress(
						packet.address,
						packet.port),
				"some motd",
				false
		);
		proxyServer.getConfig().addServer(linkedServerInfo);

		// Add the newly linked server to the list of servers that take priority in that bucket.
		List<String> serversInPriorityBucket = this.serverPriorities.get(packet.priority);
		if (serversInPriorityBucket == null) {
			serversInPriorityBucket = new ArrayList<>(1);
		}
		serversInPriorityBucket.add(packet.name);
		this.serverPriorities.put(packet.priority, serversInPriorityBucket);
		this.recomputeServerJoinOrder();

		this.logger.info("Current priority bucket " + packet.priority + ": "
				+ Arrays.toString(serversInPriorityBucket.toArray()));
	}

	@Override
	public void onUnlinkServerPacket(UnlinkServerPacket packet) {
		this.logger.info("Unlinking server " + packet);

		ServerInfo serverInfo = this.proxyServer.getServerInfo(packet.name);
		if (serverInfo == null) {
			this.logger.warning("Error: server " + packet.name + " is not registered.");
			return;
		}

		this.proxyServer.getConfig().removeServer(serverInfo);

		this.recomputeServerJoinOrder();
	}

	@Override
	public void onTransportPlayerPacket(TransportPlayerPacket packet) {
		// The logging is ugly but after picking back up this project it helped me out, /shrug

		this.logger.info("Transporting player " + packet);

		UUID uuid = UUID.fromString(packet.player);
		this.logger.info("the uuid is " + uuid);

		for (ProxiedPlayer p : this.proxyServer.getPlayers()) {
			this.logger.info("found proxied player: " + p + ", " + p.getName() + ", " + p.getServer() + ", " + p.getUniqueId());
			UUID target = p.getUniqueId();
			this.logger.info("src = " + uuid + ", target = " + target + ", src == target = " + (uuid == target));
		}

		ProxiedPlayer proxiedPlayer = this.proxyServer.getPlayer(UUID.fromString(packet.player));
		if (proxiedPlayer == null) {
			this.logger.warning("Couldn't get player " + packet.player);
			return;
		}

		ServerInfo serverInfo = this.proxyServer.getServerInfo(packet.to);
		if (serverInfo == null) {
			this.logger.warning("Couldn't get server " + packet.to);
		}

		proxiedPlayer.connect(serverInfo);
		this.logger.info("Transported player " + packet);
	}

	/**
	 * In a more sophisticated setup, we may employ some fancy logic to
	 * better distribute players to lobby servers.
	 *
	 * In this simple setup, we simply distribute them to the highest
	 * priority server.
	 *
	 * Why levels of priority? I dunno, but too lazy to change it rn >.>
	 */
	private void recomputeServerJoinOrder() {
		for (List<String> priorities : this.serverPriorities.descendingMap().values()) {
			for (String name : priorities) {
				if (name == null) continue;

				ServerInfo serverInfo = proxyServer.getServerInfo(name);
				if (serverInfo == null) continue;

				this.rejoinReconnectHandler.server = serverInfo;
				this.logger.info("Updated default reconnection server: " + serverInfo.getName());
				return;
			}
		}

		// We should always have a server to forward players to, but we don't have any...

		if (this.rejoinReconnectHandler.server != null) {
			String name = this.rejoinReconnectHandler.server.getName();
			this.logger.warning("Couldn't assign a server to `rejoinReconnectHandler` - current server is" + name);
		} else {
			this.logger.warning("Couldn't assign a server to `rejoinReconnectHandler` - no idea how players are gonna be routed lol");
		}
	}
}
