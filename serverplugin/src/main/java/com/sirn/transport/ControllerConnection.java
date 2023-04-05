package com.sirn.transport;

import com.fasterxml.jackson.core.JsonProcessingException;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.msgpack.jackson.dataformat.MessagePackFactory;

import java.io.*;
import java.net.Socket;
import java.util.logging.Logger;

import com.sirn.transport.packets.AuthenticationPacket;
import com.sirn.transport.packets.Packet;
import com.sirn.transport.packets.PongPacket;
import com.sirn.transport.packets.UpdateActivePacket;

public class ControllerConnection implements Closeable {
	private static final ObjectMapper objectMapper = new ObjectMapper(new MessagePackFactory());
	private final Logger logger;
	private final Socket socket;
    private final DataInputStream reader;
    private final DataOutputStream writer;

	public ControllerConnection(Logger logger, Socket socket) throws IOException {
		this.logger = logger;
		this.socket = socket;
        this.reader = new DataInputStream(this.socket.getInputStream());
        this.writer = new DataOutputStream(this.socket.getOutputStream());
	}

	public Packet read() throws IOException {
		int length = reader.readInt();
		byte[] bytes = new byte[length];
		reader.readFully(bytes);

		Packet packet;
		packet = objectMapper.readValue(bytes, Packet.class);
		this.logger.info("Received packet: " + packet);

		return packet;
	}

	// Type-safe methods for only the packets we're allowed to send

	public void write(AuthenticationPacket packet) throws IOException {
		Packet wrapperPacket = new Packet();
		wrapperPacket.authenticationPacket = packet;
		this.write(wrapperPacket);
	}

	public void write(PongPacket packet) throws IOException {
		Packet wrapperPacket = new Packet();
		wrapperPacket.pongPacket = packet;
		this.write(wrapperPacket);
	}

	public void write(UpdateActivePacket packet) throws IOException {
		Packet wrapperPacket = new Packet();
		wrapperPacket.updateActivePacket = packet;
		this.write(wrapperPacket);
	}

	private void write(Packet packet) throws IOException {
		this.logger.info("Writing packet: " + packet);

        byte[] payload;
        try {
            payload = objectMapper.writeValueAsBytes(packet);
        } catch (JsonProcessingException e) {
            e.printStackTrace();
            return;
        }

        this.writer.writeInt(payload.length);
        this.writer.write(payload);
	}

	@Override
	public void close() throws IOException {
		this.reader.close();
		this.writer.close();
		this.socket.close();
	}
}
