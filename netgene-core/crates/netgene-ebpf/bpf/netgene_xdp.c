// SPDX-License-Identifier: MIT OR Apache-2.0
// NetGene XDP flow counter: counts packets and bytes per IPv4 source address.
// It never drops or modifies traffic (always XDP_PASS).

#include <linux/bpf.h>
#include <linux/if_ether.h>
#include <linux/ip.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>

struct flow_stats {
    __u64 packets;
    __u64 bytes;
};

struct {
    __uint(type, BPF_MAP_TYPE_LRU_HASH);
    __uint(max_entries, 4096);
    __type(key, __u32);   /* IPv4 source address, network byte order */
    __type(value, struct flow_stats);
} flows SEC(".maps");

SEC("xdp")
int netgene_count(struct xdp_md *ctx)
{
    void *data = (void *)(long)ctx->data;
    void *data_end = (void *)(long)ctx->data_end;

    struct ethhdr *eth = data;
    if ((void *)(eth + 1) > data_end)
        return XDP_PASS;
    if (eth->h_proto != bpf_htons(ETH_P_IP))
        return XDP_PASS;

    struct iphdr *ip = (void *)(eth + 1);
    if ((void *)(ip + 1) > data_end)
        return XDP_PASS;

    __u32 src = ip->saddr;
    __u64 len = (__u64)(data_end - data);
    struct flow_stats *stats = bpf_map_lookup_elem(&flows, &src);
    if (stats) {
        __sync_fetch_and_add(&stats->packets, 1);
        __sync_fetch_and_add(&stats->bytes, len);
    } else {
        struct flow_stats init = { .packets = 1, .bytes = len };
        bpf_map_update_elem(&flows, &src, &init, BPF_NOEXIST);
    }
    return XDP_PASS;
}

char LICENSE[] SEC("license") = "Dual MIT/GPL";
