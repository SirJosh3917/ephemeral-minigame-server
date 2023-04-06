package com.sirn.proxy;

import net.md_5.bungee.api.ReconnectHandler;
import net.md_5.bungee.api.config.ServerInfo;
import net.md_5.bungee.api.connection.ProxiedPlayer;

import java.util.logging.Logger;

public class RejoinReconnectHandler implements ReconnectHandler {
    private final Logger logger;
    public ServerInfo server;

    public RejoinReconnectHandler(Logger logger) {
        this.logger = logger;
    }

    @Override
    public ServerInfo getServer(ProxiedPlayer player) {
        String name = "null";
        if (this.server != null) {
            name = this.server.getName();
        }
        this.logger.info("ReconnectHandler: sending player " + player.getName() + " to server " + name);

        return this.server;
    }

    // do nothing - we do not do any stateful manipulation

    @Override
    public void setServer(ProxiedPlayer player) {
    }

    @Override
    public void save() {
    }

    @Override
    public void close() {
    }
}
