# Traceroute 

Program takes 1 argument, IPv4 address.

Usage in command line (program may need root privileges): 
```
> ./traceroute 8.8.8.8

```

Output:
```
> ./traceroute 8.8.4.4
 Hop   Host IP address      Answer time    
  1.   192.168.0.1          4.713785ms
  2.   84.116.254.140       20.270348ms
  3.   84.116.253.129       25.506147ms
  4.   84.116.133.29        28.954009ms
  5.   84.116.138.73        27.131111ms
  6.   72.14.222.250        27.995776ms
  7.   108.170.250.209      29.643509ms
  8.   216.239.40.213       27.726382ms
  9.   8.8.8.8              27.644684ms

```