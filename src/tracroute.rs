extern crate pnet;

use pnet::util::checksum;
use pnet::packet::{
    icmp::{
        echo_reply::EchoReplyPacket,
        echo_request::{MutableEchoRequestPacket, EchoRequestPacket},
        IcmpPacket,
        IcmpType,
        IcmpTypes,
    },
    ip::IpNextHeaderProtocols,
    ipv4::MutableIpv4Packet,
    MutablePacket,
    Packet,
};
use pnet::transport::{icmp_packet_iter, transport_channel, TransportChannelType::Layer3};

use std::net::{IpAddr, Ipv4Addr};
use std::time::{Instant, Duration};


static IPV4_HEADER_LEN: u32 = 21;
static ICMP_HEADER_LEN: u32 = 8;
static ICMP_PAYLOAD_LEN: u32 = 32;
static MAX_TTL: usize = 64;


#[derive(Clone, Debug)]
struct HopReply {
    hop_addr: IpAddr,
    reply_time: Duration,
    reply_type: IcmpType,
    sequence_number: u16
}

fn create_icmp_packet<'a>(
    buf_ip: &'a mut [u8],
    buf_icmp: &'a mut [u8],
    dest: Ipv4Addr,
    ttl: u8,
    sequence_number: u16,
) -> MutableIpv4Packet<'a> {
    let mut ipv4_packet = MutableIpv4Packet::new(buf_ip)
        .expect("Error creating IPv4 packet");
    
    ipv4_packet.set_version(4);
    ipv4_packet.set_header_length(IPV4_HEADER_LEN as u8);
    ipv4_packet.set_total_length((IPV4_HEADER_LEN + ICMP_HEADER_LEN + ICMP_PAYLOAD_LEN) as u16);
    ipv4_packet.set_ttl(ttl);
    ipv4_packet.set_next_level_protocol(IpNextHeaderProtocols::Icmp);
    ipv4_packet.set_destination(dest);

    let mut icmp_packet = MutableEchoRequestPacket::new(buf_icmp)
        .expect("Error creating ICMP packet");

    icmp_packet.set_icmp_type(IcmpTypes::EchoRequest);
    icmp_packet.set_sequence_number(sequence_number);

    let checksum = checksum(&icmp_packet.packet_mut(), 1);

    icmp_packet.set_checksum(checksum);
    ipv4_packet.set_payload(icmp_packet.packet_mut());

    ipv4_packet
}

fn process_reply(reply: IcmpPacket, host: IpAddr, duration: Duration) -> Option<HopReply> {
    match reply.get_icmp_type() {
        IcmpTypes::TimeExceeded => {
            /* 
                Time exceeded message returns IP header and first 8 bytes of original datagram's payload, so..
                Original echo request is on 28 byte offset.
            */
            let request_packet = EchoRequestPacket::new(&reply.packet()[28..])
                .expect("Parsing echo request packet failed!");

            Some( HopReply {
                hop_addr: host,
                reply_time: duration,
                reply_type: IcmpTypes::TimeExceeded,
                sequence_number: request_packet.get_sequence_number(),
            })
        },
        IcmpTypes::EchoReply => {
            let reply_packet = EchoReplyPacket::new(&reply.packet())
                .expect("Parsing echo reply packet failed!");

            Some( HopReply {
                hop_addr: host,
                reply_time: duration,
                reply_type: IcmpTypes::EchoReply,
                sequence_number: reply_packet.get_sequence_number(),
            })
        },
        _ => None,
    }
}

pub fn run_traceroute(dest: Ipv4Addr, requests_per_hop: usize, wait_time: u64) {
    let (mut tx, mut rx) = transport_channel(
        1024,
        Layer3(IpNextHeaderProtocols::Icmp))
        .expect("Creating transport channel failed!");

    let mut rx = icmp_packet_iter(&mut rx);

    let mut is_destionantion = false;
    let mut ttl: usize = 1;
    let packet_time_sec = Duration::from_secs(wait_time); 

    let mut buf_ip = [0u8; 64];
    let mut buf_icmp = [0u8; 40];

    println!("{:>4}   {:<20} {:<15}", "Hop", "Host IP address", "Answer time");

    while !is_destionantion && ttl <= MAX_TTL {
        let mut replies: Vec<HopReply> = Vec::with_capacity(requests_per_hop);

        let timer_start = Instant::now();

        for i in 0..requests_per_hop {
            let icmp_packet = create_icmp_packet(
                &mut buf_ip, 
                &mut buf_icmp, 
                dest, 
                ttl as u8,
                ((ttl - 1) * requests_per_hop + i) as u16);

            tx.send_to(icmp_packet, std::net::IpAddr::V4(dest))
                .expect("Sending packet failed!");
        }

        loop {
            let waiting_time = timer_start.elapsed();

            if waiting_time > packet_time_sec { break; }

            let receiving_time = packet_time_sec - timer_start.elapsed();

            match rx.next_with_timeout(receiving_time) {
                Ok(Some((reply, host))) => {
                    /* In reply first 20 bytes encode IP header. */
                    let icmp_header = IcmpPacket::new(&reply.packet()[20..])
                        .expect("Parsing reply failed!");

                    if let Some(hop) = process_reply(icmp_header, host, timer_start.elapsed()) {
                        replies.push(hop);
                    }
                }, 
                Ok(None) => break, // time expired
                Err(err) => panic!("Receiving packet error:\n{:?}", err),
            }
        }
        
        /* Filter out all previous unhandled packets. */
        let replies: Vec<HopReply> = replies.into_iter().filter( |reply| {
            let sequence_number = reply.sequence_number as usize;
            (ttl - 1) * requests_per_hop <= sequence_number && sequence_number < ttl * requests_per_hop
        }).collect();

        /* Check we got reply from destination host. */
        is_destionantion = replies.iter().any(|reply| reply.reply_type == IcmpTypes::EchoReply);

        if replies.is_empty() {
            /* 0 received packets */
            println!("{:>3}.   {:^20} {:^15}", ttl, "*", "*");

        }
        else if replies.len() < requests_per_hop {
            /* Received less packets than were sent. */
            println!("{:>3}.   {:<20} {:^15}", ttl, replies[0].hop_addr.to_string(), "*");
        }
        else if replies.len() == requests_per_hop {
            /* Received all packets */
            let avrg_time = replies.iter()
                .fold(Duration::from_secs(0), |acc, reply| acc + reply.reply_time) / requests_per_hop as u32;

            println!("{:>3}.   {:<20} {:^15?}", ttl, replies[0].hop_addr.to_string(), avrg_time);
        }
        
        ttl += 1;
    }
    
    if ttl > MAX_TTL {
        println!("TTL value exceeded! Traceroute exits.", );
    }
    
}