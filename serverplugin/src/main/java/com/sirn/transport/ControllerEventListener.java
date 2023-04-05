package com.sirn.transport;

import java.io.IOException;

import com.sirn.transport.packets.*;

public abstract class ControllerEventListener {
	public abstract void onConnect(ControllerConnection connection) throws IOException;
	public void onDisconnect() {}

	public void onLinkServerPacket(LinkServerPacket packet) throws IOException {}
	public void onUnlinkServerPacket(UnlinkServerPacket packet) throws IOException {}
	public void onTransportPlayerPacket(TransportPlayerPacket packet) throws IOException {}
	public void onRequestPacket(RequestPacket packet) throws IOException {}
	public void onPingPacket(PingPacket packet) throws IOException {}
}
