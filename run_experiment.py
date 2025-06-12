#!/usr/bin/env python3

"""
Launch distributed setup using Ansible:
1. Starts manager services on each IP listed in the inventory file.
2. Starts the interface service on a given IP, providing it with the config file and list of manager IPs.
"""

import subprocess
import argparse

def count_non_empty_lines(file_path: str) -> int:
    """
    Counts the number of non-empty lines in a text file.

    Parameters:
        file_path (str): Path to the input text file.

    Returns:
        int: Number of non-empty lines.
    """
    with open(file_path, "r", encoding="utf-8") as f:
        return sum(1 for line in f if line.strip())


def start_managers(inventory_path: str):
    """
    Runs the Ansible playbook to start manager services.

    Args:
        inventory_path (str): Path to the Ansible inventory file listing manager machines.
    """
    subprocess.run([
        "ansible-playbook",
        "-i", inventory_path,
        "playbooks/deploy.yml",
        "--fork", f"{count_non_empty_lines(inventory_path)}"
    ], check=True)


def start_interface(interface_ip: str, config_path: str, manager_inventory: str):
    """
    Runs the Ansible playbook to start the interface on the given IP.

    Args:
        interface_ip (str): IP address of the interface machine.
        config_path (str): Path to the JSON config file.
        manager_inventory (str): Path to the inventory file listing manager machines (used as a parameter).
    """
    subprocess.run([
        "ansible-playbook", "playbooks/interface_setup.yml",
        "-i", f"{interface_ip},", 
        "-u", "root",
        "-e", f"json_config_path={config_path} machines_path={manager_inventory}"
    ], check=True)


def main():
    parser = argparse.ArgumentParser(description="Deploy a distributed system using Ansible.")
    parser.add_argument("config_path", help="Path to the JSON config file for the interface.")
    parser.add_argument("manager_inventory", help="Path to Ansible inventory file for manager nodes.")
    parser.add_argument("interface_ip", help="IP address of the interface machine.")

    args = parser.parse_args()

    print("[*] Starting manager nodes...")
    start_managers(args.manager_inventory)

    print("[*] Starting interface node...")
    start_interface(args.interface_ip, args.config_path, args.manager_inventory)


if __name__ == "__main__":
    main()
