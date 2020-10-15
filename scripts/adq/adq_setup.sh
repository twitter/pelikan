#!/bin/bash

# https://blog.cloudflare.com/how-to-achieve-low-latency/
# https://null.53bits.co.uk/index.php?page=numa-and-queue-affinity



ICE_PATH=/home/junchengy/adq/ice/
iface=eth0
num_instance=24
port=12300
num_queues_tc0=2
num_queues_tc1=32
num_queues_tc2=32
tc1offset=${num_queues_tc0}
# ipaddr=10.181.156.119
host=$(hostname)
ipaddr=$(ifconfig|grep "inet "|grep broadcast|awk '{print $2}')


general_tuning()
{
    # disable firewall
    service firewalld stop; systemctl mask firewalld

    # Enable latency-performance tuned profile
    tuned-adm profile latency-performance
    # check profile
    cat /etc/tuned/active_profile

    # Set the CPU scaling governor to performance mode
    x86_energy_perf_policy --hwp-enable 2>/dev/null
    x86_energy_perf_policy performance

    # change ulimit
    grep "hard nofile 65536" /etc/security/limits.conf
    status=$?
    if [[ "$status" == 1 ]]; then
      echo "modify limits.conf"
      echo -e '* hard nofile 65536'"\n"'* soft nofile 65536' | sudo tee -a /etc/security/limits.conf
    fi


    sysctl -w net.core.busy_poll=50000
    sysctl -w net.core.busy_read=50000
    sysctl -w net.core.somaxconn=4096
    sysctl -w net.core.netdev_max_backlog=8192
    sysctl -w net.ipv4.tcp_max_syn_backlog=16384
    sysctl -w net.core.rmem_max=16777216
    sysctl -w net.core.wmem_max=16777216
    sysctl -w net.ipv4.tcp_mem="764688 1019584 16777216"
    sysctl -w net.ipv4.tcp_rmem="8192 87380 16777216"
    sysctl -w net.ipv4.tcp_wmem="8192 65536 16777216"
    sysctl -w net.ipv4.route.flush=1

    # echo 800000 | sudo tee /proc/sys/net/ipv4/tcp_max_orphans


    # Stop the irqbalance service. (Needed for interface interrupt affinity settings.)
    systemctl stop irqbalance
    echo kernel.numa_balancing=0 | sudo tee -a /etc/sysctl.conf
    sysctl -p
}

offline_cores() {
  # assume we only keep 25 cores
  # cat /proc/cpuinfo | grep -e processor -e "core id" -e "physical id"
  lscpu
  nproc=`nproc`
  start=$((num_instance+1))
  end=$((nproc-1))
  for i in `seq ${start} ${end}`; do
    echo 0 | sudo tee /sys/devices/system/cpu/cpu${i}/online
  done
}

turnoff_service() {
  sudo puppet-util setbranch off

  sudo systemctl stop rezolus
  sudo systemctl stop monit
  sudo systemctl stop ntpdate
  sudo systemctl stop osqueryd
  sudo systemctl stop pcscd
  sudo systemctl stop rsyslog
  sudo systemctl stop scribe
  sudo systemctl stop splunk
  sudo systemctl stop tricorder
  sudo systemctl stop tss-host-daemon
  sudo systemctl stop twitcher
  sudo systemctl stop twitterfw
  sudo systemctl stop vexd
  sudo systemctl stop fleetexec-server
  sudo systemctl stop absorber.tss.production.host-daemon.13399.service
}


non_adq()
{
  # set wilson attributes to disable IRQ affinity
  loony -H $host -d server set attribute irqaffinity:false

  # check current queues
  ethtool -l $iface

  # find IRQ for the nic
  ls /sys/class/net/$iface/device/msi_irqs/

  # spread processing evenly between first 25 RX queues, and disable the other queues
  sudo ethtool -X $iface equal 25
  # check the queue binding
  sudo ethtool -x $iface


  # IRQ affinity
  let CPU=0
  cd /sys/class/net/$iface/device/msi_irqs/ || exit 1
  for IRQ in *; do
    cat /proc/irq/$IRQ/smp_affinity_list;
     echo $CPU > /proc/irq/$IRQ/smp_affinity_list
    ((CPU+=1))
  done

  # Flow steering: this is used for initial mapping (connection start)
  for ((i=0; i < num_queues_tc1+1; i++)); do
    ethtool --config-ntuple $iface flow-type tcp4 dst-port $((port + i)) action $i
    sleep 0.5
  done

  # receive flow steering, this is used for continuous packet steering
  echo 3276800 > /proc/sys/net/core/rps_sock_flow_entries
  for f in /sys/class/net/$iface/queues/rx-*/rps_flow_cnt; do
    echo 32768 > $f;
  done

  # Mellanox, turn on accelerated receive flow steering
  ethtool -K $iface ntuple on

  # xps
  ${ICE_PATH}/PROCGB/Linux/ice-1.0.4/scripts/set_xps_rxqs $iface

}





adq()
{
    modprobe ice

    # Enable hardware TC offload on the interface and turn off lldp
    ethtool -K $iface hw-tc-offload on
    ethtool --set-priv-flags $iface fw-lldp-agent off

    # verify settings
    ethtool -k $iface | grep "hw-tc"
    ethtool --show-priv-flags $iface

    read -s -n 1 -p "check hw-tc-offload is on before continue"

    /opt/iproute2/sbin/tc qdisc add dev $iface root mqprio num_tc 3 map 0 1 2 queues $num_queues_tc0@0 $num_queues_tc1@$num_queues_tc0 $num_queues_tc2@$((num_queues_tc0 + num_queues_tc1)) hw 1 mode channel
    sleep 8
    /opt/iproute2/sbin/tc qdisc add dev $iface ingress
    sleep 8

    # create TC filters: one per pelikan instance
    for ((i = 0; i < num_queues_tc1; i++)); do
      /opt/iproute2/sbin/tc filter add dev $iface protocol ip ingress prio 1 flower dst_ip $ipaddr/32 ip_proto tcp dst_port $((port + i)) skip_sw hw_tc 1
    done
    sleep 8

    # check filter
    /opt/iproute2/sbin/tc qdisc show dev $iface
    /opt/iproute2/sbin/tc qdisc show dev $iface ingress

    # Set the interrupt moderation rate to a static value for Tx and turn off interrupt moderation for Rx
    ethtool --coalesce ${iface} adaptive-rx off rx-usecs 0
    ethtool --coalesce ${iface} adaptive-tx off tx-usecs 500
    sleep 1

    # config Intel Ethernet Flow Director (so that no two threads busy poll the same queue)
    ethtool --features $iface ntuple on
    ethtool --set-priv-flags $iface channel-inline-flow-director off
    sleep 1

    for ((i=0; i < num_queues_tc1; i++)); do
      ethtool --config-ntuple $iface flow-type tcp4 dst-port $((port + i)) action $(((i % num_queues_tc1) + tc1offset))
      sleep 0.5
    done

    # check whether it is set correctly
    ethtool --show-ntuple $iface
    read -s -n 1 -p "check tc before continue"

    # Run the set_irq_affinity script for all interfaces
    ${ICE_PATH}/PROCGB/Linux/ice-1.0.4/scripts/set_irq_affinity -X all $iface
    # Configure symmetric queues on the interface
    ${ICE_PATH}/PROCGB/Linux/ice-1.0.4/scripts/set_xps_rxqs $iface


    # create cgroup
    sudo yum -y install libcgroup libcgroup-tools
    cgroup_name="app_tc1"
    cgcreate -g net_prio:${cgroup_name}
    cgset -r net_prio.ifpriomap="$iface 1" ${cgroup_name}

    cat /sys/fs/cgroup/net_prio/${cgroup_name}/net_prio.ifpriomap
    cat /sys/fs/cgroup/net_prio/${cgroup_name}/tasks

    # cgexec -g net_prio:${cgroup_name} --sticky $command
}


cleanup() {
    # clean up
    jobs -p | xargs kill &> /dev/null
    cgdelete -g net_prio:${cgroup_name}
}


watch() {
    watch -d -n 0.5 "ethtool -S $iface | grep busy | column"
}


general_tuning
offline_cores
turnoff_service

# non_adq
adq

