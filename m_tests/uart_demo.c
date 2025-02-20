#define readSize 20

volatile char *uart = (volatile char *)0x10000000;

void println(char *string);
char *readln();
int main()
{
    char *string = readln();
    println(string);
    return 0;
}

void println(char *string)
{
    int i = 0;
    while (string[i] != '\0')
    {
        uart[0] = string[i];
        i++;
    }
    uart[0] = '\n';
}

char *readln()
{
    static char buffer[readSize];
    char letter;
    int i = 0;
    while (letter != '\0' && i < readSize)
    {
        // polling uart for next letter
        while ((uart[5] & 0x01) == 0)
            ;
        buffer[i] = uart[0];
        letter = buffer[i];
        i++;
    }

    return (char *)&buffer;
}