package com.sirn.server;

import java.io.IOException;
import java.util.logging.Logger;

import com.sirn.transport.ControllerConnection;
import com.sirn.transport.ControllerEventListener;
import com.sirn.transport.packets.*;

public class ServerPacketListener extends ControllerEventListener {
	private final Logger logger;
	private final AuthenticationPacket authenticationPacket;

	public ServerPacketListener(Logger logger, AuthenticationPacket authenticationPacket) {
		this.logger = logger;
		this.authenticationPacket = authenticationPacket;
	}

	public ControllerConnection connection;
	private boolean acceptingPlayers = false;

	@Override
	public void onConnect(ControllerConnection connection) throws IOException {
		this.connection = connection;
		this.connection.write(this.authenticationPacket);
		this.logger.info("Authentication packet sent");
	}

	@Override
	public void onDisconnect() {
		this.logger.info("Proxy disconnected!");
		this.logger.info("TODO: Implement more robust logic to repair things upon disconnecting");
	}

	@Override
	public void onPingPacket(PingPacket packet) throws IOException {
        this.logger.info("Received ping " + packet + ", accepting players?: " + this.acceptingPlayers);

        if (this.acceptingPlayers) {
            this.connection.write(new PongPacket(packet.timer));
        }
	}

    public boolean isAcceptingPlayers() {
        return this.acceptingPlayers;
    }

    public void setAcceptingPlayers(boolean value) {
        this.acceptingPlayers = value;

		try {
        	this.connection.write(new UpdateActivePacket(value));
		} catch (IOException e) {
			// It's fine to ignore any errors here, as sending this packet is
			// merely an optimistic/optimism thing.
		}
    }
}
