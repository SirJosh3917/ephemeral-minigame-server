package com.sirn.transport;

import com.sirn.transport.packets.*;

public abstract class ControllerEventListener {
	public abstract void onConnect(ControllerConnection connection);
	public void onDisconnect() {}

	public void onLinkServerPacket(LinkServerPacket packet) {}
	public void onUnlinkServerPacket(UnlinkServerPacket packet) {}
	public void onTransportPlayerPacket(TransportPlayerPacket packet) {}
	public void onRequestPacket(RequestPacket packet) {}
	public void onPingPacket(PingPacket packet) {}
}
