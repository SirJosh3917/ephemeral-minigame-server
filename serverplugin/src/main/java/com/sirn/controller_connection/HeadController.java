package com.sirn.controller_connection;

import com.fasterxml.jackson.core.JsonProcessingException;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.sirn.packets.Packet;
import com.sirn.packets.PingPacket;
import com.sirn.packets.PongPacket;
import com.sirn.packets.UpdateActivePacket;
import com.sirn.packets.AuthenticationPacket;

import org.msgpack.jackson.dataformat.MessagePackFactory;

import java.io.*;
import java.net.Socket;
import java.util.logging.Logger;

public class HeadController implements Closeable {
    private final ObjectMapper objectMapper = new ObjectMapper(new MessagePackFactory());
    private final Socket socket;
    private final DataInputStream reader;
    private final DataOutputStream writer;
    private final Thread messageHandlerThread;
    private final Logger logger;
    private final AuthenticationPacket authenticationPacket;
    private boolean acceptingPlayers = true;

    public HeadController(Logger logger, AuthenticationPacket authenticationPacket, Socket socket) throws IOException {
        this.logger = logger;
        this.authenticationPacket = authenticationPacket;
        this.socket = socket;
        this.reader = new DataInputStream(this.socket.getInputStream());
        this.writer = new DataOutputStream(this.socket.getOutputStream());
        this.messageHandlerThread = new Thread(this::run);
        messageHandlerThread.start();
    }

    protected void onPing(PingPacket packet) {
        this.logger.info("Received ping " + packet + ", accepting players?: " + this.acceptingPlayers);

        if (this.acceptingPlayers) {
            this.send(new Packet(new PongPacket(packet.timer)));
        }
    }

    public boolean isAcceptingPlayers() {
        return this.acceptingPlayers;
    }

    public void setAcceptingPlayers(boolean value) {
        this.acceptingPlayers = value;
        this.send(new Packet(new UpdateActivePacket(value)));
    }

    @Override
    public void close() throws IOException {
        this.socket.close();
    }

    public void send(Packet packet) {
        this.logger.info("Sending packet to HQ: " + packet);

        byte[] payload;
        try {
            payload = this.objectMapper.writeValueAsBytes(packet);
        } catch (JsonProcessingException e) {
            e.printStackTrace();
            return;
        }

        try {
            synchronized (this.writer) {
                this.writer.writeInt(payload.length);
                this.writer.write(payload);
                this.writer.flush();
            }
        } catch (IOException e) {
            e.printStackTrace();
        }
    }

    protected void run() {
        {
            Packet packet = new Packet();
            packet.authenticationPacket = this.authenticationPacket;
            this.send(packet);
        }

        DataInputStream reader = this.reader;

        while (this.socket.isConnected() && !this.socket.isInputShutdown()) {
            try {
                int length = reader.readInt();
                byte[] bytes = new byte[length];
                reader.readFully(bytes);

                Packet packet;
                packet = this.objectMapper.readValue(bytes, Packet.class);

                if (packet.pingPacket != null) {
                    this.onPing(packet.pingPacket);
                } else {
                    System.out.println("uhh couldn't deser packet... ?");
                }
            } catch (IOException e) {
                e.printStackTrace();
                System.out.println("io exception");
            }
        }
    }
}
