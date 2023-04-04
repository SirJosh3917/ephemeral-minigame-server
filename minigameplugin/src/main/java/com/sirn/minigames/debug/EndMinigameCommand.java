package com.sirn.minigames.debug;

import org.bukkit.Server;
import org.bukkit.command.Command;
import org.bukkit.command.CommandExecutor;
import org.bukkit.command.CommandSender;

public class EndMinigameCommand implements CommandExecutor {
    private final Server server;

    public EndMinigameCommand(Server server) {
        this.server = server;
    }

    @Override
    public boolean onCommand(CommandSender sender, Command command, String label, String[] args) {
        sender.sendMessage("okay executing");
        server.broadcast("helo we weil be shutting down :(", "admin");
        server.shutdown();
        return true;
    }
}
