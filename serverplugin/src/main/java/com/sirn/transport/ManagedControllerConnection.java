package com.sirn.transport;

import java.io.IOException;
import java.net.Socket;
import java.util.logging.Logger;

import com.sirn.transport.packets.Packet;

public class ManagedControllerConnection {
	public static interface SocketFactory {
		public Socket create() throws IOException;
	}

	private final Logger logger;
	private final SocketFactory factory;
	private final ControllerEventListener listener;
	private final Thread thread;

	public ManagedControllerConnection(Logger logger, SocketFactory factory, ControllerEventListener listener) {
		this.logger = logger;
		this.factory = factory;
		this.listener = listener;
		this.thread = new Thread(this::run);
		this.thread.start();
	}

	private void run() {
		final int MAX_BACKOFF_SECONDS = 30;
		int backoff = 1;

		while (true) {

			Socket socket;
			try {
				this.logger.info("Connecting to controller...");
				socket = this.factory.create();
				backoff = 1;
			} catch (IOException e) {
				this.logger.info("Failed to connect to controller, backing off for " + backoff + " seconds");
				e.printStackTrace();

				try { Thread.sleep(backoff * 1000); } catch (InterruptedException e2) {}

				backoff *= 2;
				if (backoff > MAX_BACKOFF_SECONDS) {
					backoff = MAX_BACKOFF_SECONDS;
				}
				continue;
			}

			try (ControllerConnection connection = new ControllerConnection(this.logger, socket)) {
				this.listener.onConnect(connection);

				while (true) {
					Packet packet = connection.read();

					// These cases intentionally unhandled as they are C -> S:
					//
					// - authenticationPacket
					// - requestPacket
					// - pongPacket
					// - updateActivePacket

					if (packet.linkServerPacket != null) {
						listener.onLinkServerPacket(packet.linkServerPacket);
					} else if (packet.unlinkServerPacket != null) {
						listener.onUnlinkServerPacket(packet.unlinkServerPacket);
					} else if (packet.transportPlayerPacket != null) {
						listener.onTransportPlayerPacket(packet.transportPlayerPacket);
					} else if (packet.pingPacket != null) {
						listener.onPingPacket(packet.pingPacket);
					} else {
						// Probably should be an exception, but /shrug
						this.logger.info("Unknown packet received from controller... ???");
					}
				}
			} catch (IOException e) {
				this.listener.onDisconnect();
				this.logger.info("Error while talking to controller");
				e.printStackTrace();
			}
		}
	}
}
