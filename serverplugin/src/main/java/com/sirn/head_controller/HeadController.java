package com.sirn.head_controller;

import com.fasterxml.jackson.core.JsonProcessingException;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.sirn.packets.*;
import com.sirn.packets.AuthenticationKind;
import com.sirn.packets.AuthenticationPacket;
import com.sirn.packets.LinkServerPacket;
import com.sirn.packets.Packet;
import com.sirn.packets.TransportPlayerPacket;
import com.sirn.packets.UnlinkServerPacket;

import net.md_5.bungee.api.ProxyConfig;
import net.md_5.bungee.api.ProxyServer;
import net.md_5.bungee.api.config.ListenerInfo;
import net.md_5.bungee.api.config.ServerInfo;
import net.md_5.bungee.api.connection.ProxiedPlayer;
import org.msgpack.jackson.dataformat.MessagePackFactory;

import java.io.*;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.util.*;

public class HeadController implements Closeable {
    private final ProxyServer proxyServer;
    private final ObjectMapper objectMapper = new ObjectMapper(new MessagePackFactory());
    private final Socket socket;
    private final DataInputStream reader;
    private final DataOutputStream writer;
    private final Thread messageHandlerThread;
    private final TreeMap<Integer, List<String>> serverPriorities;
    public final RejoinReconnectHandler rejoinReconnectHandler;

    public HeadController(ProxyServer proxyServer, Socket socket) throws IOException {
        this.proxyServer = proxyServer;
        this.socket = socket;
        this.serverPriorities = new TreeMap<>();
        this.rejoinReconnectHandler = new RejoinReconnectHandler(proxyServer.getLogger());
        this.reader = new DataInputStream(this.socket.getInputStream());
        this.writer = new DataOutputStream(this.socket.getOutputStream());
        this.messageHandlerThread = new Thread(this::run);
        messageHandlerThread.start();
    }

    protected void onLinkServer(LinkServerPacket packet) {
        this.proxyServer.getLogger().info("Linking connected server " + packet);

        // create server and link it
        ServerInfo linkedServerInfo = this.proxyServer.constructServerInfo(
                packet.name,
                new InetSocketAddress(
                        packet.address,
                        packet.port),
                "some motd",
                false
        );

        proxyServer.getConfig().addServer(linkedServerInfo);

        // update priority buckets
        List<String> serversInPriorityBucket = this.serverPriorities.get(packet.priority);

        if (serversInPriorityBucket == null) {
            serversInPriorityBucket = new ArrayList<>();
        }

        serversInPriorityBucket.add(packet.name);
        this.serverPriorities.put(packet.priority, serversInPriorityBucket);

        this.proxyServer.getLogger().info("Current priority bucket " + packet.priority + ": "
                + Arrays.toString(serversInPriorityBucket.toArray()));

        // update priority list
        updatePriorityList();
    }

    protected void onUnlinkServer(UnlinkServerPacket packet) {
        this.proxyServer.getLogger().info("Unlinking server " + packet);

        // delete server from waterfall
        ServerInfo serverInfo = this.proxyServer.getServerInfo(packet.name);

        if (serverInfo == null) {
            this.proxyServer.getLogger().warning("Error: server " + packet.name + " is not registered.");
            return;
        }

        this.proxyServer.getConfig().removeServer(serverInfo);

        // update priority list
        this.updatePriorityList();
    }

    protected void onTransportPlayer(TransportPlayerPacket packet) {
        this.proxyServer.getLogger().info("Transporting player " + packet);

        UUID uuid = UUID.fromString(packet.player);
        this.proxyServer.getLogger().info("the uuid is " + uuid);

        for (ProxiedPlayer p : this.proxyServer.getPlayers()) {
            this.proxyServer.getLogger().info("found proxied player: " + p + ", " + p.getName() + ", " + p.getServer() + ", " + p.getUniqueId());
            UUID target = p.getUniqueId();
            this.proxyServer.getLogger().info("src = " + uuid + ", target = " + target + ", src == target = " + (uuid == target));
        }

        ProxiedPlayer proxiedPlayer = this.proxyServer.getPlayer(UUID.fromString(packet.player));
        if (proxiedPlayer == null) {
            this.proxyServer.getLogger().warning("Couldn't get player " + packet.player);
            return;
        }

        ServerInfo serverInfo = this.proxyServer.getServerInfo(packet.to);
        if (serverInfo == null) {
            this.proxyServer.getLogger().warning("Couldn't get server " + packet.to);
        }

        proxiedPlayer.connect(serverInfo);
        this.proxyServer.getLogger().info("Transported player " + packet);
    }

    private void updatePriorityList() {
        for (List<String> priorities : this.serverPriorities.descendingMap().values()) {
            for (String name : priorities) {
                if (name == null) continue;

                ServerInfo serverInfo = proxyServer.getServerInfo(name);
                if (serverInfo == null) continue;

                this.rejoinReconnectHandler.server = serverInfo;
                this.proxyServer.getLogger().info("Updated default reconnection server: " + serverInfo.getName());
                return;
            }
        }

        // failure case: we should've returned early
        String name = "null";

        if (this.rejoinReconnectHandler.server != null) {
            name = this.rejoinReconnectHandler.server.getName();
        }

        this.proxyServer.getLogger().warning("Couldn't assign a server to `rejoinReconnectHandler` - current server is"
                + name);
    }

    @Override
    public void close() throws IOException {
        this.socket.close();
    }

    private void send(Packet packet) {
        byte[] payload;
        try {
            payload = this.objectMapper.writeValueAsBytes(packet);
        } catch (JsonProcessingException e) {
            e.printStackTrace();
            return;
        }

        DataOutputStream writer = this.writer;

        try {
            writer.writeInt(payload.length);
            writer.write(payload);
        } catch (IOException e) {
            e.printStackTrace();
        }
    }

    protected void run() {
        this.proxyServer.getLogger().info("Connection thread started!");

        {
            AuthenticationKind authenticationKind = new AuthenticationKind();
            authenticationKind.tag = "Proxy";

            AuthenticationPacket authenticationPacket = new AuthenticationPacket();
            authenticationPacket.name = "proxy";
            authenticationPacket.kind = authenticationKind;

            ListenerInfo listenerInfo = this.proxyServer.getConfig().getListeners().iterator().next();
            assert listenerInfo != null;
            authenticationPacket.ip = listenerInfo.getSocketAddress().toString();

            Packet packet = new Packet();
            packet.authenticationPacket = authenticationPacket;
            this.send(packet);
            this.proxyServer.getLogger().info("Authentication packet sent");
        }

        DataInputStream reader = this.reader;

        while (this.socket.isConnected() && !this.socket.isClosed()) {
            try {
                int length = reader.readInt();
                byte[] bytes = new byte[length];
                reader.readFully(bytes);

                Packet packet;
                packet = this.objectMapper.readValue(bytes, Packet.class);
                this.proxyServer.getLogger().info("Received packet: " + packet);

                if (packet.linkServerPacket != null) {
                    this.onLinkServer(packet.linkServerPacket);
                } else if (packet.unlinkServerPacket != null) {
                    this.onUnlinkServer(packet.unlinkServerPacket);
                } else if (packet.transportPlayerPacket != null) {
                    this.onTransportPlayer(packet.transportPlayerPacket);
                } else {
                    System.out.println("uhh couldn't deser packet... ?");
                }
            } catch (IOException e) {
                e.printStackTrace();
                System.out.println("io exception");
            }
        }

        this.proxyServer.getLogger().severe("Shutting down controller connection thread");
    }
}
