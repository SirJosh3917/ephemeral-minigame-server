package com.sirn.commands;

import com.sirn.controller_connection.HeadController;
import com.sirn.controller_connection.packets.Packet;
import org.bukkit.command.Command;
import org.bukkit.command.CommandExecutor;
import org.bukkit.command.CommandSender;
import org.bukkit.entity.Player;

import java.util.Arrays;
import java.util.UUID;

public class RequestCommand implements CommandExecutor {
    private final HeadController connection;

    public RequestCommand(HeadController connection) {
        this.connection = connection;
    }

    @Override
    public boolean onCommand(CommandSender sender, Command command, String label, String[] args) {
        sender.sendMessage("request invoked with args " + Arrays.toString(args));

        String player = null;

        if (sender instanceof Player) {
            UUID uuid = ((Player) sender).getUniqueId();
            player = uuid.toString();
        }

        Packet request = Packet.makeRequestMinigame(args[0], player);
        this.connection.send(request);
        sender.sendMessage("you will be sent to a minigame server shortly (if you are not, try again)");

        return true;
    }
}
